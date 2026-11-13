import { describe, expect, it } from "vitest";

import { RENDERER_ROOT, runTimeGraph } from "../../../test/runTimeGraph";
import { EARTH_CASE } from "./cases";
import type { ThickReferenceCases } from "./thickCases";
import { thickCasesReplyOf } from "./thickCasesMessages";

/** The four thick cases' names, each standing on `earth`'s case, which builds no Mie. */
function standInReferenceCases(): ThickReferenceCases {
  return {
    "venus-92bar": EARTH_CASE,
    "venus-58bar": EARTH_CASE,
    "titan-class": EARTH_CASE,
    "giant-deck": EARTH_CASE,
  };
}

describe("the thick cases' worker's reply", () => {
  it("is failed with the build's own message where the build throws", () => {
    expect(
      thickCasesReplyOf(
        () => {
          throw new Error("no NH3 index at 645 nm");
        },
        () => ({}),
      ),
    ).toEqual({ kind: "failed", message: "no NH3 index at 645 nm" });
  });

  it("is failed with the covers' own message where the build lands and the covers throw (R08.T15.b.2)", () => {
    expect(
      thickCasesReplyOf(
        () => standInReferenceCases(),
        () => {
          throw new Error("R05's Earth has no tabulated deck mode aerosol:H2SO4");
        },
      ),
    ).toEqual({ kind: "failed", message: "R05's Earth has no tabulated deck mode aerosol:H2SO4" });
  });
});

describe("the thick cases' worker's graph", () => {
  const graph = [...runTimeGraph(`${RENDERER_ROOT}view/atmosphere/reference/thickCases.worker.ts`)];

  it("is read from the sources it walks", () => {
    expect(graph).toContain(`${RENDERER_ROOT}view/atmosphere/reference/thickCases.ts`);
  });

  it("loads no engine, hillaire.ts, tables.ts or scene at run time", () => {
    const engine = graph.filter(
      (path) =>
        path.includes("/view/engine/") ||
        path.includes("/view/scenes/") ||
        path.includes("/view/scene/") ||
        path.endsWith("/view/atmosphere/hillaire.ts") ||
        path.endsWith("/view/atmosphere/tables.ts"),
    );
    expect(engine).toEqual([]);
  });
});
