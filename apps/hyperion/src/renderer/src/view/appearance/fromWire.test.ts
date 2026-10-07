import type { BodyPhotometryDto, BodySummaryDto, PhaseTemplateDto } from "@hyperion/protocol";
import { describe, expect, it, vi } from "vitest";

import { toSystemBodiesModel } from "../../lib/system/bodiesWire";
import type { SystemBody } from "../../lib/system/model";
import { FIXTURE_EARTH, sliceBodies, sliceBodiesWith } from "../../test/planetaryFixture";
import {
  appearanceFromWire,
  BOND_RATIO_TOLERANCE,
  type BondRatioFinding,
  HELD_PHOTOMETRIES_KEPT,
  heldAppearanceOf,
  heldPhotometryOf,
  modelledPhotometry,
  PROVISIONAL_LABELS,
  PROVISIONAL_PHOTOMETRY,
  reportBondRatioFinding,
} from "./fromWire";
import { discIntegratedPhase, geometricAlbedo } from "./phase";
import { lambertPhase, phaseIntegral } from "./shapes";

/** The generator's q for the `earth` template, `photometry/templates.golden` at version 21. */
const GENERATOR_EARTH_Q = 1.311572861542696;

/** The fixture's Earth as the client models it, its summary changed by `change`. */
function fixtureEarth(
  change: (earth: BodySummaryDto) => BodySummaryDto = (earth) => earth,
): SystemBody {
  const answer = sliceBodiesWith((body) => (body.id === FIXTURE_EARTH ? change(body) : body));
  const result = toSystemBodiesModel(answer, "H7K 4C0RFZ D-7");
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  const earth = result.bodies.bodies.find((body) => body.id === FIXTURE_EARTH);
  if (earth === undefined) {
    throw new Error("no Earth in the fixture");
  }
  return earth;
}

/** The fixture's Earth's photometric section on the wire. */
function earthPhotometry(): BodyPhotometryDto {
  const section = sliceBodies().bodies.find((body) => body.id === FIXTURE_EARTH)?.photometry;
  if (section?.state !== "ok") {
    throw new Error("the fixture's Earth has a photometric section");
  }
  return section.value;
}

/** The fixture's Earth with its photometric section changed by `change`. */
function earthWithPhotometry(change: Partial<BodyPhotometryDto>): SystemBody {
  return fixtureEarth((earth) => ({
    ...earth,
    photometry: { state: "ok", value: { ...earthPhotometry(), ...change } },
  }));
}

/** The fixture's Earth's own law's ratio, p_V q_V ÷ A_Bond. */
function earthLawRatio(): number {
  const earth = fixtureEarth();
  if (earth.photometry.state !== "ok") {
    throw new Error("the fixture's Earth has a photometric section");
  }
  return modelledPhotometry(earth.photometry.value).lawRatio;
}

describe("PROVISIONAL_PHOTOMETRY", () => {
  it("is a sphere of geometric albedo 0.2", () => {
    for (const p of geometricAlbedo(PROVISIONAL_PHOTOMETRY.law)) {
      expect(p).toBeCloseTo(0.2, 12);
    }
  });

  it("has a Lambert sphere's phase integral 3 ÷ 2", () => {
    const q = phaseIntegral((alpha) => discIntegratedPhase(PROVISIONAL_PHOTOMETRY.law, alpha)[1]);
    expect(Math.abs(q / 1.5 - 1)).toBeLessThan(5e-4);
  });

  it("follows Lambert's phase function", () => {
    for (const alpha of [0.3, 1, 2, 2.8]) {
      expect(discIntegratedPhase(PROVISIONAL_PHOTOMETRY.law, alpha)[1]).toBeCloseTo(
        lambertPhase(alpha),
        4,
      );
    }
  });

  it("is labelled provisional", () => {
    expect(PROVISIONAL_PHOTOMETRY.provenance).toBe("provisional");
    expect(PROVISIONAL_PHOTOMETRY.bondRatioCheck).toBeNull();
  });
});

describe("appearanceFromWire with a photometric section", () => {
  it("shades the body with the law its section states, modelled and unlabelled", () => {
    const { photometry, labels } = appearanceFromWire(fixtureEarth());

    expect(photometry.provenance).toBe("modelled");
    expect(photometry.law).toEqual({
      a: [0.21 * 1.5, 0.215 * 1.5, 0.263 * 1.5],
      lommelSeeligerShare: 0,
      template: "earth",
      phaseExponent: [1, 1, 1],
    });
    expect(labels).toEqual([]);
  });

  it("takes B, V and R as the display's b, g and r", () => {
    expect(appearanceFromWire(fixtureEarth()).photometry.geometricAlbedo).toEqual([
      0.21, 0.215, 0.263,
    ]);
  });

  it("finds q from the law, the generator's Earth's to 10⁻⁶", () => {
    for (const q of appearanceFromWire(fixtureEarth()).photometry.phaseIntegral) {
      expect(Math.abs(q / GENERATOR_EARTH_Q - 1)).toBeLessThan(1e-6);
    }
  });

  it("carries the stated ratio as a check", () => {
    expect(appearanceFromWire(fixtureEarth()).photometry.bondRatioCheck).toBe(0.95914);
  });

  it("solves nothing: the section's exponents are the law's", () => {
    const law = appearanceFromWire(
      earthWithPhotometry({ phase_exponent: { b: 1.2, v: 1, r: 0.9 } }),
    ).photometry.law;

    expect(law.phaseExponent).toEqual([0.9, 1, 1.2]);
  });

  it("takes the section's L, and A = p ÷ [L + ⅔(1 − L)]", () => {
    const law = appearanceFromWire(
      earthWithPhotometry({ phase_template: "mars", lunar_lambert_share: 0.5 }),
    ).photometry.law;

    expect(law.lommelSeeligerShare).toBe(0.5);
    expect(law.a[1]).toBeCloseTo(0.215 / (0.5 + (2 / 3) * 0.5), 15);
  });

  it.each([
    ["moon", "moon"],
    ["mercury", "mercury"],
    ["mars", "mars"],
    ["venus", "venus"],
    ["earth", "earth"],
    ["jupiter", "jupiter"],
    ["saturn", "saturn"],
    ["uranus", "uranus"],
    ["neptune", "neptune"],
    ["airless_ice", "airless-ice"],
    ["snowball", "snowball"],
    ["magma", "magma"],
  ] as const)("maps the wire's %s template to the client's %s", (wire, client) => {
    const template: PhaseTemplateDto = wire;
    expect(
      appearanceFromWire(earthWithPhotometry({ phase_template: template })).photometry.law.template,
    ).toBe(client);
  });
});

describe("appearanceFromWire without a photometric section", () => {
  it.each(["not_resolved", "not_modelled", "not_applicable"] as const)(
    "takes the provisional law and label for a section %s",
    (state) => {
      const appearance = appearanceFromWire(
        fixtureEarth((earth) => ({ ...earth, photometry: { state } })),
      );

      expect(appearance.photometry).toBe(PROVISIONAL_PHOTOMETRY);
      expect(appearance.labels).toEqual(PROVISIONAL_LABELS);
      expect(appearance.bondRatioFinding).toBeNull();
    },
  );

  it("takes the provisional law and label for an older server's summary without the field", () => {
    const appearance = appearanceFromWire(
      fixtureEarth(({ photometry: _photometry, ...older }) => older),
    );

    expect(appearance.photometry).toBe(PROVISIONAL_PHOTOMETRY);
    expect(appearance.labels).toEqual(["BODY PHOTOMETRY: NOT YET MODELLED"]);
  });
});

describe("appearanceFromWire's figure", () => {
  it("is the figure section's spheroid about its pole", () => {
    expect(appearanceFromWire(fixtureEarth()).figure).toEqual({
      equatorialRadiusM: 6_378_137,
      polarRadiusM: 6_356_752.314245179,
      pole: { x: 0, y: -0.39769200800981236, z: 0.9175189735177814 },
    });
  });

  it("is a sphere of the mean radius about the rotation's pole without a figure section", () => {
    const earth = fixtureEarth((body) => ({ ...body, figure: { state: "not_modelled" } }));
    const radiusM = earth.bulk.state === "ok" ? earth.bulk.value.radiusM : Number.NaN;

    expect(appearanceFromWire(earth).figure).toEqual({
      equatorialRadiusM: radiusM,
      polarRadiusM: radiusM,
      pole: { x: 0, y: -0.39769200800981236, z: 0.9175189735177814 },
    });
  });

  it("is a sphere about no pole with neither a figure nor a rotation", () => {
    const earth = fixtureEarth((body) => ({
      ...body,
      figure: { state: "not_modelled" },
      rotation: { state: "not_modelled" },
    }));

    expect(appearanceFromWire(earth).figure?.pole).toBeNull();
  });

  it("is none for a body without a granted radius", () => {
    const earth = fixtureEarth((body) => ({
      ...body,
      bulk: { state: "not_resolved" },
      figure: { state: "not_resolved" },
    }));

    expect(appearanceFromWire(earth).figure).toBeNull();
  });
});

describe("the stated ratio against the law's (Design note 5)", () => {
  it("agrees for the fixture's Earth, 0.215 × 1.3116 ÷ 0.294", () => {
    expect(Math.abs(earthLawRatio() / ((0.215 * GENERATOR_EARTH_Q) / 0.294) - 1)).toBeLessThan(
      1e-6,
    );
    expect(appearanceFromWire(fixtureEarth()).bondRatioFinding).toBeNull();
  });

  it.each([1 - BOND_RATIO_TOLERANCE + 0.01, 1 + BOND_RATIO_TOLERANCE - 0.01])(
    "is no finding at %s of the law's",
    (share) => {
      const body = earthWithPhotometry({ bond_ratio: earthLawRatio() / share });
      expect(appearanceFromWire(body).bondRatioFinding).toBeNull();
    },
  );

  it.each([1 - BOND_RATIO_TOLERANCE - 0.01, 1 + BOND_RATIO_TOLERANCE + 0.01])(
    "is a finding for plan 14's owner at %s of the law's",
    (share) => {
      const statedRatio = earthLawRatio() / share;
      const body = earthWithPhotometry({ bond_ratio: statedRatio });
      expect(appearanceFromWire(body).bondRatioFinding).toEqual({
        statedRatio,
        lawRatio: earthLawRatio(),
      });
    },
  );

  it("is a finding where the Bond albedo is 0, which no law's ratio can meet", () => {
    const body = earthWithPhotometry({ bond_albedo: 0 });
    expect(appearanceFromWire(body).bondRatioFinding?.lawRatio).toBe(Number.POSITIVE_INFINITY);
  });
});

describe("a section marked provisional (P14.T47.b)", () => {
  it("is modelled-provisional, its law the section's", () => {
    const photometry = appearanceFromWire(
      earthWithPhotometry({ phase_template: "airless_ice", provisional: true }),
    ).photometry;
    expect([photometry.provenance, photometry.law.template]).toEqual([
      "modelled-provisional",
      "airless-ice",
    ]);
  });

  it("states no label: no view labels a flagged section (decision-r07-provisional-photometry)", () => {
    const appearance = appearanceFromWire(earthWithPhotometry({ provisional: true }));
    expect(appearance.labels).toEqual([]);
  });
});

describe("heldPhotometryOf", () => {
  it("makes one photometry for equal sections of two records", () => {
    const first = fixtureEarth();
    const again = fixtureEarth();
    if (first.photometry.state !== "ok" || again.photometry.state !== "ok") {
      throw new Error("the fixture's Earth has a photometric section");
    }

    expect(again).not.toBe(first);
    expect(heldPhotometryOf(again.photometry.value).photometry.law).toBe(
      heldPhotometryOf(first.photometry.value).photometry.law,
    );
  });

  it(`keeps the ${String(HELD_PHOTOMETRIES_KEPT)} sections used most recently`, () => {
    const section = (ratio: number) => {
      const body = earthWithPhotometry({ bond_ratio: ratio });
      if (body.photometry.state !== "ok") {
        throw new Error("the fixture's Earth has a photometric section");
      }
      return body.photometry.value;
    };
    // Ratios no other test states, so that the module's cache holds none of them before.
    const kept = heldPhotometryOf(section(7.25));
    const dropped = heldPhotometryOf(section(7.5));
    for (let i = 1; i < HELD_PHOTOMETRIES_KEPT; i += 1) {
      heldPhotometryOf(section(7.25));
      heldPhotometryOf(section(8 + i / 1_000));
    }

    expect(heldPhotometryOf(section(7.25))).toBe(kept);
    expect(heldPhotometryOf(section(7.5))).not.toBe(dropped);
  });
});

describe("heldAppearanceOf", () => {
  it("makes a record's appearance once, so that its law is one object", () => {
    const earth = fixtureEarth();
    const first = heldAppearanceOf(earth);

    expect(heldAppearanceOf(earth)).toBe(first);
    expect(heldAppearanceOf(earth).photometry.law).toBe(first.photometry.law);
  });

  it("logs nothing, a finding included, so that a render may call it", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);

    const appearance = heldAppearanceOf(earthWithPhotometry({ bond_ratio: earthLawRatio() / 1.2 }));

    expect(appearance.bondRatioFinding).not.toBeNull();
    expect(warn).not.toHaveBeenCalled();
  });
});

describe("reportBondRatioFinding", () => {
  /** A finding no other test makes: the fixture's Earth stated 10% under its law. */
  function tenPercentFinding(): BondRatioFinding {
    const finding = appearanceFromWire(
      earthWithPhotometry({ bond_ratio: earthLawRatio() / 1.1 }),
    ).bondRatioFinding;
    if (finding === null) {
      throw new Error("a ratio 10% from the law's is a finding");
    }
    return finding;
  }

  it("warns once a body and finding, naming the body, its departure and plan 14's owner", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const finding = tenPercentFinding();

    reportBondRatioFinding("TEST BODY A", finding);
    reportBondRatioFinding("TEST BODY A", finding);

    expect(warn).toHaveBeenCalledOnce();
    expect(warn.mock.calls[0]?.[0]).toMatch(
      /^body TEST BODY A: .*\(10\.0%\).*a finding for plan 14's owner$/,
    );
  });

  it("warns again for another body with an equal section", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const finding = tenPercentFinding();

    reportBondRatioFinding("TEST BODY B", finding);
    reportBondRatioFinding("TEST BODY C", finding);

    expect(warn).toHaveBeenCalledTimes(2);
  });
});
