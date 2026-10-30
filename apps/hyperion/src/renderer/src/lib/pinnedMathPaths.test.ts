/**
 * The modules on the committed tables' paths compute their transcendentals through
 * `pinnedMath.ts` (plan R08, R08.T20): none calls a transcendental of `Math` or raises a
 * number to a power with `**`, but for `** 2` and `** 0.5`, which V8 computes exactly as `x * x` and
 * `Math.sqrt(x)` on every platform.
 *
 * @remarks
 * Each module is read as its text (`?raw`), as `reference/cases.test.ts` reads its directory, and
 * scanned with its comments and string literals blanked. `Math.hypot`, which V8 computes from its
 * own arithmetic and `Math.sqrt`, and the exact `Math` functions are allowed. `.oxlintrc.json` has
 * no rule that can do this. A module joins the list when a committed table, or a test's bit-exact
 * pin, comes to read it; every test that writes a committed table through `test/bless.ts` is in it,
 * which the scan checks over every renderer test.
 */
import { describe, expect, it } from "vitest";

/** The modules whose output a committed table or a bit-exact pin holds, by path from here. */
const MODULES = import.meta.glob<string>(
  [
    // The research's list (plan R08, R08.T20).
    "../view/atmosphere/reference/geometries.ts",
    "../view/atmosphere/reference/twin.ts",
    "../view/atmosphere/reference/caseGate.ts",
    "../view/atmosphere/column.ts",
    "../view/atmosphere/aerosol.ts",
    "../view/atmosphere/sizeDistribution.ts",
    "../view/atmosphere/nonSpherical.ts",
    "../view/atmosphere/absorbers.ts",
    "../view/atmosphere/absorbers/hMinus.ts",
    "../view/atmosphere/earth.ts",
    "../view/atmosphere/medium.ts",
    "../view/atmosphere/mie.ts",
    "../view/atmosphere/tablesCpu.ts",
    "../view/atmosphere/hillaire.ts",
    "../test/atmosphereSweep.ts",
    "../test/atmosphereCases.ts",
    "../test/mieMatrices.ts",
    // The modules whose platform maths a one-ulp nudge found moving a committed table's bits.
    "../view/atmosphere/forwardLobe.ts",
    "../view/atmosphere/forwardPeak.ts",
    "../view/atmosphere/gate.ts",
    "../view/atmosphere/irradianceQuadrature.ts",
    "../view/atmosphere/oblate.ts",
    "../view/atmosphere/opticalDepth.ts",
    "../view/atmosphere/rayleigh.ts",
    "../view/atmosphere/reference/references.ts",
    "../view/atmosphere/tables.ts",
    "../view/atmosphere/thick/regime.ts",
    "../view/atmosphere/absorbers/kDistribution.ts",
    "../view/atmosphere/materials/materials.ts",
    "../test/atmosphereFixtures.ts",
    "../view/lighting/quadrature.ts",
    "../view/lighting/hostDisc.ts",
    // The thick bake's, which the by-hand records `thickRuns.json` and `vectorGate.json` read.
    "../view/atmosphere/thick/bake.ts",
    "../view/atmosphere/thick/discreteOrdinates.ts",
    "../view/atmosphere/thick/expSum.ts",
    "../view/atmosphere/thick/polarisation.ts",
    "../view/atmosphere/thick/read.ts",
    "../view/atmosphere/thick/spectralColumn.ts",
    "../view/atmosphere/thick/sphericalBeam.ts",
    // Three more whose platform maths moved no bit under the nudge, pinned at no cost: the channel
    // fit, which `channels.json` records, the Earth line paths' fixture, and the sweep boundary's
    // near-sun test, which `sweepBoundary.json`'s classes read.
    "../view/atmosphere/channels.ts",
    "../test/earthLinePaths.ts",
    "../view/atmosphere/reference/sweepBoundary.ts",
    // The tone curve and exposure `gate.ts` reads for its colours.
    "../view/photometry/toneCurve.ts",
    "../view/photometry/exposure.ts",
    // Every test that writes a committed table through `test/bless.ts`, whose own bodies compute
    // some of their tables' values.
    "../test/bless.test.ts",
    "../test/mieMatrices.test.ts",
    "../view/atmosphere/absorbers.test.ts",
    "../view/atmosphere/absorbers/earthLines.test.ts",
    "../view/atmosphere/channels.test.ts",
    "../view/atmosphere/mieSharedCases.test.ts",
    "../view/atmosphere/reference/absorberCurves.test.ts",
    "../view/atmosphere/reference/caseGate.test.ts",
    "../view/atmosphere/reference/cases.test.ts",
    "../view/atmosphere/reference/metricVerdicts.test.ts",
    "../view/atmosphere/reference/references.test.ts",
    "../view/atmosphere/reference/sweepBoundary.test.ts",
    "../view/atmosphere/reference/sweepCases.test.ts",
    "../view/atmosphere/reference/thickRuns.test.ts",
    "../view/atmosphere/reference/vectorGate.test.ts",
    // R05's descent profile, whose poses its tests pin by hash.
    "../view/spike/descentProfile.ts",
  ],
  { query: "?raw", import: "default", eager: true },
);

/** How many modules the list names: the glob drops a path that no longer exists. */
const LISTED = 60;

/** Every renderer test, as its text: the scan finds among them the tests that bless a table. */
const TESTS = import.meta.glob<string>("../**/*.test.ts", {
  query: "?raw",
  import: "default",
  eager: true,
});

/** An import of the bless helper, from any directory. */
const BLESS_IMPORT = /from "(?:\.\.?\/)+(?:test\/)?bless"/u;

/** The transcendental functions of `Math` whose results depend on the platform. */
const PLATFORM_MATH =
  /\bMath\s*\.\s*(?:exp|expm1|log|log1p|log2|log10|sin|cos|tan|asin|acos|atan|atan2|sinh|cosh|tanh|asinh|acosh|atanh|cbrt|pow)\b/gu;

/** A character blanked: a line break kept, anything else a space. */
function blank(c: string): string {
  return c === "\n" ? "\n" : " ";
}

/**
 * `source` with its comments and the contents of its string and template literals replaced by
 * spaces, line breaks kept, so that a match's offset gives its line.
 *
 * @remarks
 * It knows no regular-expression literal, so one holding a quote, a backtick or a comment's
 * opener would open a literal that never closes and blank the rest of the module; such a module
 * is refused rather than passed on what the scan could not see.
 *
 * @throws Error where `source` ends inside a string, a template or a block comment.
 */
function codeOf(source: string): string {
  const out: string[] = [];
  /** The quote a string is open in, or a template's depth of `${`, innermost last. */
  const stack: string[] = [];
  let i = 0;
  while (i < source.length) {
    const c = source.charAt(i);
    const next = source.charAt(i + 1);
    const inside = stack.at(-1);
    if (inside === '"' || inside === "'" || inside === "`") {
      if (c === "\\") {
        out.push(" ", blank(next));
        i += 2;
        continue;
      }
      if (c === inside) {
        stack.pop();
        out.push(c);
      } else if (inside === "`" && c === "$" && next === "{") {
        stack.push("{");
        out.push("${");
        i += 2;
        continue;
      } else {
        out.push(blank(c));
      }
      i += 1;
      continue;
    }
    if (c === "/" && next === "/") {
      const end = source.indexOf("\n", i);
      const stop = end < 0 ? source.length : end;
      out.push(" ".repeat(stop - i));
      i = stop;
      continue;
    }
    if (c === "/" && next === "*") {
      const end = source.indexOf("*/", i + 2);
      if (end < 0) {
        throw new Error("the scan reads a block comment that never closes");
      }
      const stop = end + 2;
      out.push(source.slice(i, stop).replaceAll(/[^\n]/gu, " "));
      i = stop;
      continue;
    }
    if (c === '"' || c === "'" || c === "`") {
      stack.push(c);
    } else if (c === "{" && inside === "{") {
      stack.push("{");
    } else if (c === "}" && inside === "{") {
      stack.pop();
    }
    out.push(c);
    i += 1;
  }
  if (stack.length > 0) {
    throw new Error(`the scan ends inside ${String(stack.at(-1))}: a literal it cannot read`);
  }
  return out.join("");
}

/** Whether the code after a `**` begins with the exponent 2 or 0.5 and nothing that binds to it. */
function exemptExponent(after: string): boolean {
  return /^\s*(?:2|0\.5)(?![\w.]|\s*\*\*)/u.test(after);
}

/** The line of each use of the platform's maths in `source`, as `<line>: <text>`. */
function platformMathIn(source: string): string[] {
  const code = codeOf(source);
  const lines = source.split("\n");
  const lineOf = (offset: number): string => {
    const line = code.slice(0, offset).split("\n").length;
    return `${String(line)}: ${(lines[line - 1] ?? "").trim()}`;
  };
  const uses: string[] = [];
  for (const match of code.matchAll(PLATFORM_MATH)) {
    uses.push(lineOf(match.index));
  }
  for (const match of code.matchAll(/\*\*(=?)/gu)) {
    if (match[1] === "=" || !exemptExponent(code.slice(match.index + 2))) {
      uses.push(lineOf(match.index));
    }
  }
  return uses;
}

describe("the committed tables' paths", () => {
  it("lists every module, each found", () => {
    expect(Object.keys(MODULES)).toHaveLength(LISTED);
  });

  it("lists every test that writes a committed table through bless.ts", () => {
    const writers = Object.keys(TESTS).filter((file) => BLESS_IMPORT.test(TESTS[file] ?? ""));
    expect(writers.length).toBeGreaterThanOrEqual(15);
    expect(writers.filter((file) => !(file in MODULES))).toEqual([]);
  });

  it.each(Object.keys(MODULES))("%s computes its transcendentals through pinnedMath", (file) => {
    expect(platformMathIn(MODULES[file] ?? "")).toEqual([]);
  });
});

describe("the scan", () => {
  it("finds Math's transcendentals and every other power", () => {
    const source = [
      "const a = Math.exp(x);",
      "const b = Math . atan2(y, x);",
      "const c = x ** 3;",
      "const d = x ** 2.5;",
      "const e = x ** -2;",
      "const f = 2 ** k;",
      "y **= 3;",
      "const g = x ** 2 ** k;",
    ].join("\n");
    expect(platformMathIn(source).map((use) => use.split(":")[0])).toEqual([
      "1",
      "2",
      "3",
      "4",
      "5",
      "6",
      "7",
      "8",
      "8",
    ]);
  });

  it("passes squares, square roots, the exact functions, comments and strings", () => {
    const source = [
      "const a = x ** 2 + y ** 0.5 * z;",
      "const b = Math.sqrt(Math.abs(Math.hypot(x, y)) + Math.floor(x));",
      "// Math.exp(x) and x ** 3 in a comment",
      "/* x ** 1.5, Math.sin(t) */",
      'const s = "Math.cos(t) ** 4";',
      "const t = `a ${x ** 2} Math.tan`;",
      "const m = myMath.exp(x);",
    ].join("\n");
    expect(platformMathIn(source)).toEqual([]);
  });

  it("refuses a module it cannot read to its end, such as a regular expression with a quote", () => {
    expect(() => platformMathIn('const quote = /"/u;\nconst a = Math.exp(x);')).toThrow(
      /cannot read/u,
    );
    expect(() => platformMathIn("const a = 1; /* never closed")).toThrow(/never closes/u);
  });

  it("scans the code inside a template literal's substitutions", () => {
    expect(platformMathIn("const t = `${Math.log(x)} and ${y ** 3}`;")).toHaveLength(2);
  });
});
