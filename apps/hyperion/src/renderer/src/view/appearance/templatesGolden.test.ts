/**
 * The client's phase templates against the generator's `photometry/templates.golden` (plan 14,
 * P14.T47.d; plan R07, T2.b and T4.d): the law a body's photometric section names, at the
 * template's own exponents and every pinned L, is the generator's in Φ_c at every whole degree and
 * in q, so that a modelled body is drawn with the q plan 14 states its ratio with, and through the
 * shader's f table (`discIntegratedPhase`) to `f32`. The file is read from the repository, so that
 * a re-blessed golden reaches this test at once; it is the golden after P14.T47.e (version 21),
 * whose `earth` block the client's `earth` must equal (decision-r07-earth-albedo).
 */
import type { PhaseTemplateDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import templatesGolden from "../../../../../../../crates/hyperion-sim/tests/golden/photometry/templates.golden?raw";
import type { Rgb } from "../photometry/toneCurve";
import { type PhaseTemplateId, type PhotometricLaw, phaseFactor } from "./law";
import { discIntegratedPhase, lawFor, lawPhaseIntegral } from "./phase";
import { shapePhase } from "./shapes";
import { PHASE_TEMPLATES } from "./templates";

/** Φ_c agrees with the generator's to this, absolutely: the same closed forms, other `sin`s. */
const PHI_TOLERANCE = 1e-12;

/** The shader's table's Φ agrees with the generator's to this, relatively: its f is `f32`. */
const TABLE_TOLERANCE = 1e-6;

/**
 * q agrees with the generator's to this, relatively: its Simpson sum has 1,800 intervals, the
 * client's {@link lawPhaseIntegral} 7,200, and both meet the clamp's kink.
 */
const Q_TOLERANCE = 1e-6;

/** One L block of a template: its q and Φ at 0°–180° in B, V and R. */
interface GoldenShare {
  readonly share: number;
  readonly q: readonly [number, number, number];
  readonly phi: ReadonlyArray<readonly [number, number, number]>;
}

/** One template block of the golden. */
interface GoldenTemplate {
  readonly name: string;
  readonly rowShare: number;
  readonly validToRad: number;
  readonly exponents: readonly [number, number, number];
  readonly shares: ReadonlyArray<GoldenShare>;
}

function bandsOf(text: string): [number, number, number] {
  const [b = Number.NaN, v = Number.NaN, r = Number.NaN] = text.split(",").map(Number);
  return [b, v, r];
}

/** The value after `key=` in a golden line's words. */
function valueOf(words: ReadonlyArray<string>, key: string): string {
  const word = words.find((each) => each.startsWith(`${key}=`));
  if (word === undefined) {
    throw new Error(`a golden line has no ${key}`);
  }
  return word.slice(key.length + 1);
}

/** Every template block of the golden, in its order. */
function goldenTemplates(): ReadonlyArray<GoldenTemplate> {
  const templates: {
    name: string;
    rowShare: number;
    validToRad: number;
    exponents: [number, number, number];
    shares: { share: number; q: [number, number, number]; phi: [number, number, number][] }[];
  }[] = [];
  for (const line of templatesGolden.split("\n")) {
    const words = line.split(" ");
    if (line.startsWith("template ")) {
      templates.push({
        name: words[1] ?? "",
        rowShare: Number(valueOf(words, "share")),
        validToRad: Number(valueOf(words, "valid_to")),
        exponents: bandsOf(valueOf(words, "s")),
        shares: [],
      });
    } else if (line.startsWith("l=")) {
      templates.at(-1)?.shares.push({
        share: Number(valueOf(words, "l")),
        q: bandsOf(valueOf(words, "q")),
        phi: [],
      });
    } else if (line.startsWith("a=")) {
      templates
        .at(-1)
        ?.shares.at(-1)
        ?.phi.push(bandsOf(valueOf(words, "phi")));
    }
  }
  return templates;
}

/** The client's template of a golden name, the wire's `PhaseTemplateDto`. */
function clientTemplate(name: string): PhaseTemplateId {
  const names: Readonly<Record<PhaseTemplateDto, PhaseTemplateId>> = {
    moon: "moon",
    mercury: "mercury",
    mars: "mars",
    venus: "venus",
    earth: "earth",
    jupiter: "jupiter",
    saturn: "saturn",
    uranus: "uranus",
    neptune: "neptune",
    airless_ice: "airless-ice",
    snowball: "snowball",
    magma: "magma",
  };
  const id = Object.entries(names).find(([wire]) => wire === name)?.[1];
  if (id === undefined) {
    throw new Error(`the golden names a template the wire has not: ${name}`);
  }
  return id;
}

/** The law of a template at L with its golden exponents (B, V, R as the display's b, g, r). */
function lawOf(template: GoldenTemplate, share: number): PhotometricLaw {
  const [sB, sV, sR] = template.exponents;
  return {
    a: [1, 1, 1],
    lommelSeeligerShare: share,
    template: clientTemplate(template.name),
    phaseExponent: [sR, sV, sB],
  };
}

/** The client's Φ_c at `degrees` in B, V and R: the exact f times Φ_shape, as `channel_phase`. */
function clientPhi(law: PhotometricLaw, degrees: number): [number, number, number] {
  const alpha = (degrees * Math.PI) / 180;
  const [r, g, b] = phaseFactor(law, alpha);
  const shape = shapePhase(law.lommelSeeligerShare, alpha);
  return [b * shape, g * shape, r * shape];
}

/** `rgb` in B, V and R. */
function bvr(rgb: Rgb): [number, number, number] {
  return [rgb[2], rgb[1], rgb[0]];
}

const TEMPLATES = goldenTemplates();

/**
 * The whole degrees at which a template's fit changes piece (Mallama and Hilton 2018, eqs. 8–9 and
 * 11–12, `templates.ts`); Venus's join, 163.7°, falls on no whole degree.
 */
const JOINS: Readonly<Record<string, number>> = { jupiter: 12, saturn: 6 };

/** A fit's step between its two pieces at its join, relative: 3.7 × 10⁻⁴ (Jupiter), 7.5 × 10⁻⁴. */
const JOIN_STEP_TOLERANCE = 1e-3;

/** The largest relative departure of the shader's table's Φ from the golden's, where `at` holds. */
function worstTableDeparture(at: (template: GoldenTemplate, degrees: number) => boolean): number {
  let worst = 0;
  for (const template of TEMPLATES) {
    for (const { share, phi } of template.shares) {
      const law = lawOf(template, share);
      for (const [degrees, pinned] of phi.entries()) {
        if (!at(template, degrees)) {
          continue;
        }
        const ours = bvr(discIntegratedPhase(law, (degrees * Math.PI) / 180));
        for (const band of [0, 1, 2] as const) {
          // Φ is 0 at 180° in both, where a ratio says nothing.
          if (pinned[band] > 0) {
            worst = Math.max(worst, Math.abs(ours[band] / pinned[band] - 1));
          }
        }
      }
    }
  }
  return worst;
}

describe("the client's templates against the generator's photometry/templates.golden", () => {
  it("are the golden's twelve, in its order, each with three shares of 181 degrees", () => {
    expect(TEMPLATES.map((template) => clientTemplate(template.name))).toEqual([
      "moon",
      "mercury",
      "mars",
      "venus",
      "earth",
      "jupiter",
      "saturn",
      "uranus",
      "neptune",
      "airless-ice",
      "snowball",
      "magma",
    ]);
    for (const template of TEMPLATES) {
      expect(template.shares.map(({ share, phi }) => [share, phi.length])).toEqual([
        [0, 181],
        [0.5, 181],
        [1, 181],
      ]);
    }
  });

  it("hold each template's own L and range", () => {
    for (const template of TEMPLATES) {
      const client = PHASE_TEMPLATES[clientTemplate(template.name)];
      expect([template.name, client.lommelSeeligerShare]).toEqual([
        template.name,
        template.rowShare,
      ]);
      expect(Math.abs(client.validToRad - template.validToRad)).toBeLessThan(1e-15);
    }
  });

  it("give the generator's Φ at every whole degree for every L", () => {
    let worst = 0;
    for (const template of TEMPLATES) {
      for (const { share, phi } of template.shares) {
        const law = lawOf(template, share);
        for (const [degrees, pinned] of phi.entries()) {
          const ours = clientPhi(law, degrees);
          for (const band of [0, 1, 2] as const) {
            worst = Math.max(worst, Math.abs(ours[band] - pinned[band]));
          }
        }
      }
    }
    expect(worst).toBeLessThan(PHI_TOLERANCE);
  });

  it("give the generator's Φ through the shader's table at every whole degree, to f32", () => {
    // Whole degrees fall on the 0.5° table's nodes, so the table's f differs from the exact f by
    // its f32 rounding alone, away from a piecewise fit's join.
    const worst = worstTableDeparture((template, degrees) => JOINS[template.name] !== degrees);
    expect(worst).toBeLessThan(TABLE_TOLERANCE);
  });

  it("give it within a fit's own step at a piecewise template's join", () => {
    // At Jupiter's 12° and Saturn's 6° the table's node, i × π ÷ 360, and the golden's degree,
    // d × π ÷ 180, round to either side of the join: the step between the paper's two pieces.
    const worst = worstTableDeparture((template, degrees) => JOINS[template.name] === degrees);
    expect(worst).toBeGreaterThan(TABLE_TOLERANCE);
    expect(worst).toBeLessThan(JOIN_STEP_TOLERANCE);
  });

  it("give the generator's q for every L", () => {
    let worst = 0;
    for (const template of TEMPLATES) {
      for (const { share, q } of template.shares) {
        const ours = bvr(lawPhaseIntegral(lawOf(template, share)));
        for (const band of [0, 1, 2] as const) {
          worst = Math.max(worst, Math.abs(ours[band] / q[band] - 1));
        }
      }
    }
    expect(worst).toBeLessThan(Q_TOLERANCE);
  });

  it("recover each template's exponents from its q at its own L through lawFor", () => {
    for (const template of TEMPLATES) {
      const own = template.shares.find(({ share }) => share === template.rowShare);
      if (own === undefined) {
        throw new Error(`${template.name}'s own L is pinned`);
      }
      const [qB, qV, qR] = own.q;
      const law = lawFor([0.2, 0.2, 0.2], [qR, qV, qB], clientTemplate(template.name));
      const [sR, sV, sB] = law.phaseExponent;
      const [goldenB, goldenV, goldenR] = template.exponents;
      expect([template.name, sB, sV, sR]).toEqual([
        template.name,
        expect.closeTo(goldenB, 6),
        expect.closeTo(goldenV, 6),
        expect.closeTo(goldenR, 6),
      ]);
    }
  });

  it("make the client's earth the golden's earth block, Robinson 2026's eq. 14", () => {
    const earth = TEMPLATES.find((template) => template.name === "earth");
    if (earth === undefined) {
      throw new Error("the golden has an earth block");
    }
    const law = lawOf(earth, 0);
    const pinned = earth.shares[0];
    expect(pinned?.share).toBe(0);
    for (const [degrees, phi] of (pinned?.phi ?? []).entries()) {
      expect(Math.abs(clientPhi(law, degrees)[1] - phi[1])).toBeLessThan(PHI_TOLERANCE);
    }
    expect(bvr(lawPhaseIntegral(law))[1] / (pinned?.q[1] ?? Number.NaN) - 1).toBeCloseTo(0, 6);
  });
});
