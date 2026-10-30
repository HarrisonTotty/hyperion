import { describe, expect, it } from "vitest";

import { RENDERER_ROOT, runTimeGraph } from "../../../test/runTimeGraph";
import { thickCasesReplyOf } from "./thickCasesMessages";

describe("the thick cases' worker's reply", () => {
  it("is failed with the build's own message where the build throws", () => {
    expect(
      thickCasesReplyOf(() => {
        throw new Error("no NH3 index at 645 nm");
      }),
    ).toEqual({ kind: "failed", message: "no NH3 index at 645 nm" });
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
