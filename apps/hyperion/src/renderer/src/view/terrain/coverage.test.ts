import type { CoverDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import codesGolden from "../../../../../../../crates/hyperion-surface/tests/golden/wire/codes.golden?raw";
import type { AllocationEvent } from "../engine/memory";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { stToUv } from "./cube";
import {
  COVERAGE_MASK_SURVEYED,
  CoverageMask,
  coverageMaskTextureSpec,
  CoverageView,
  fieldCellAt,
  fieldCellCount,
  fieldCellIndex,
  resolutionOfCodeM,
} from "./coverage";
import type { Face, PatchKey } from "./patchKey";

/** The IEEE 754 bits of `x`, `0x` and 16 hexadecimal digits, as the golden writes them. */
function bitsHex(x: number): string {
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, x);
  return `0x${view.getBigUint64(0).toString(16).padStart(16, "0")}`;
}

/** The golden's `resolution_m[c]` rows: the bits, or `none`, by code. */
function goldenResolutions(): Map<number, string> {
  const rows = new Map<number, string>();
  for (const line of codesGolden.split("\n")) {
    const match = /^resolution_m\[(\d+)\] = (none|0x[0-9a-f]{16})/u.exec(line);
    if (match !== null) {
      rows.set(Number(match[1]), match[2] ?? "");
    }
  }
  return rows;
}

function cover(...ranges: ReadonlyArray<readonly [number, number, number]>): CoverDto {
  return { ranges: ranges.map(([start, end, resolution]) => ({ start, end, resolution })) };
}

/** The index of cell (i, j) of `face` at level 5. */
function cell(face: Face, i: number, j: number): number {
  return fieldCellIndex({ face, level: 5, i, j });
}

function merged(view: CoverageView, c: CoverDto, revision: number): CoverageView {
  const result = view.merged(c, revision);
  if (!result.ok) {
    throw new Error(result.message);
  }
  return result.coverage;
}

/** The solid angle of a cell of face 0, sr, by the midpoint rule over its gnomonic rectangle. */
function integratedSolidAngleSr(level: number, i: number, j: number): number {
  const n = 2 ** level;
  const u0 = stToUv(i / n);
  const u1 = stToUv((i + 1) / n);
  const v0 = stToUv(j / n);
  const v1 = stToUv((j + 1) / n);
  const steps = 64;
  let sum = 0;
  for (let a = 0; a < steps; a += 1) {
    for (let b = 0; b < steps; b += 1) {
      const u = u0 + ((a + 0.5) * (u1 - u0)) / steps;
      const v = v0 + ((b + 0.5) * (v1 - v0)) / steps;
      sum += 1 / (1 + u * u + v * v) ** 1.5;
    }
  }
  return (sum * (u1 - u0) * (v1 - v0)) / (steps * steps);
}

describe("resolution codes", () => {
  it("give R09's resolution_m for every code, bit for bit", () => {
    const rows = goldenResolutions();
    expect(rows.size).toBe(256);
    for (let code = 0; code <= 255; code += 1) {
      const resolution = resolutionOfCodeM(code);
      expect(resolution === null ? "none" : bitsHex(resolution), `code ${code}`).toBe(
        rows.get(code),
      );
    }
  });

  it("refuse a code that is not a byte", () => {
    expect(() => resolutionOfCodeM(256)).toThrow(/byte/u);
    expect(() => resolutionOfCodeM(1.5)).toThrow(/byte/u);
  });
});

describe("coarse cell indices", () => {
  it("are the face then the Morton code of i and j, as R09's cell_index", () => {
    // i = 3 (0b011) on the even bits, j = 5 (0b101) on the odd: 0b100111.
    expect(fieldCellIndex({ face: 2, level: 5, i: 3, j: 5 })).toBe(2 * 4 ** 5 + 0b10_0111);
    expect(fieldCellIndex({ face: 5, level: 8, i: 255, j: 255 })).toBe(fieldCellCount(8) - 1);
  });

  it("give a cell's children the four indices after four times its own", () => {
    const parent: PatchKey = { face: 4, level: 5, i: 17, j: 9 };
    const n = fieldCellIndex(parent);
    const children = [0, 1, 2, 3].map((k) =>
      fieldCellIndex({ face: 4, level: 6, i: 2 * 17 + (k & 1), j: 2 * 9 + (k >> 1) }),
    );
    expect(children).toEqual([4 * n, 4 * n + 1, 4 * n + 2, 4 * n + 3]);
  });

  it("are undone by fieldCellAt for every cell of a level", () => {
    for (let index = 0; index < fieldCellCount(5); index += 1) {
      expect(fieldCellIndex(fieldCellAt(5, index))).toBe(index);
    }
  });
});

describe("a body's coverage", () => {
  it("keeps each cell's finest code and leaves margin cells unsurveyed", () => {
    let view = CoverageView.empty(5);
    view = merged(view, cover([0, 4, 40], [4, 6, 0]), 1);
    view = merged(view, cover([2, 3, 60], [3, 5, 20]), 2);
    const at = (index: number): number | null => view.resolutionAtM(fieldCellAt(5, index));
    expect(at(0)).toBe(resolutionOfCodeM(40));
    expect(at(2)).toBe(resolutionOfCodeM(40));
    expect(at(3)).toBe(resolutionOfCodeM(20));
    expect(at(4)).toBe(resolutionOfCodeM(20));
    expect(at(5)).toBeNull();
    expect(at(6)).toBeNull();
    expect(view.revision).toBe(2);
    expect(view.bestResolutionM()).toBe(resolutionOfCodeM(20));
  });

  it("keeps the larger revision, since a body's only rises", () => {
    const view = merged(merged(CoverageView.empty(5), cover(), 3), cover(), 2);
    expect(view.revision).toBe(3);
  });

  it("reads a patch deeper than the field at its coarse ancestor, and refuses a coarser one", () => {
    const view = merged(CoverageView.empty(5), cover([0, 1, 9]), 1);
    expect(view.resolutionAtM({ face: 0, level: 12, i: 127, j: 127 })).toBe(resolutionOfCodeM(9));
    expect(view.resolutionAtM({ face: 0, level: 12, i: 128, j: 0 })).toBeNull();
    expect(() => view.resolutionAtM({ face: 0, level: 4, i: 0, j: 0 })).toThrow(/coarser/u);
  });

  it("refuses a range that is not a run of the level's cells, or a code that is not a byte", () => {
    const empty = CoverageView.empty(5);
    expect(empty.merged(cover([3, 3, 1]), 1)).toMatchObject({ ok: false });
    expect(empty.merged(cover([0, fieldCellCount(5) + 1, 1]), 1)).toMatchObject({ ok: false });
    expect(empty.merged(cover([0, 1, 300]), 1)).toMatchObject({ ok: false });
  });

  it("has no best resolution before any cell is surveyed", () => {
    expect(merged(CoverageView.empty(6), cover([0, 9, 0]), 0).bestResolutionM()).toBeNull();
  });

  it("states the surveyed share of the surface by solid angle, not by cell count", () => {
    const whole = merged(CoverageView.empty(5), cover([0, fieldCellCount(5), 30]), 1);
    expect(whole.surveyedFraction()).toBe(1);
    const allButOne = merged(CoverageView.empty(5), cover([1, fieldCellCount(5), 30]), 1);
    expect(allButOne.surveyedFraction()).toBeLessThan(1);
    expect(allButOne.surveyedFraction()).toBeCloseTo(1, 3);
    expect(CoverageView.empty(5).surveyedFraction()).toBe(0);
    const oneFace = merged(CoverageView.empty(5), cover([4 ** 5, 2 * 4 ** 5, 30]), 1);
    expect(oneFace.surveyedFraction()).toBeCloseTo(1 / 6, 12);
    for (const [i, j] of [
      [0, 0],
      [16, 16],
      [31, 7],
    ] as const) {
      const index = cell(0, i, j);
      const one = merged(CoverageView.empty(5), cover([index, index + 1, 30]), 1);
      expect(one.surveyedFraction() * 4 * Math.PI).toBeCloseTo(integratedSolidAngleSr(5, i, j), 8);
    }
    const corner = merged(CoverageView.empty(5), cover([0, 1, 30]), 1).surveyedFraction();
    expect(corner).toBeLessThan(1 / fieldCellCount(5));
  });

  it("marks stale without copying what it holds", () => {
    const view = merged(CoverageView.empty(5), cover([0, 2, 9]), 1);
    const stale = view.withStale(true);
    expect(stale.stale).toBe(true);
    expect(view.stale).toBe(false);
    expect(stale.resolutionAtM(fieldCellAt(5, 1))).toBe(resolutionOfCodeM(9));
    expect(stale.withStale(true)).toBe(stale);
  });
});

describe("the GPU coverage mask", () => {
  it("is a byte a cell a face, face then row j then column i, surveyed cells set", () => {
    const margin = cell(1, 4, 4);
    const view = merged(
      CoverageView.empty(5),
      cover(
        [cell(0, 0, 0), cell(0, 0, 0) + 1, 12],
        [cell(3, 31, 2), cell(3, 31, 2) + 1, 1],
        [margin, margin + 1, 0],
      ),
      1,
    );
    const texels = view.maskTexels();
    expect(texels).toHaveLength(6 * 32 * 32);
    const texel = (face: number, i: number, j: number): number | undefined =>
      texels[(face * 32 + j) * 32 + i];
    expect(texel(0, 0, 0)).toBe(COVERAGE_MASK_SURVEYED);
    expect(texel(3, 31, 2)).toBe(COVERAGE_MASK_SURVEYED);
    expect(texel(1, 4, 4)).toBe(0);
    expect(texels.reduce((n, t) => n + (t === 0 ? 0 : 1), 0)).toBe(2);
  });

  it("is made under coarse-field-gpu, 384 KiB at level 8, and remade for another level", async () => {
    const engine = await countingRenderEngine();
    const events: AllocationEvent[] = [];
    engine.onAllocation((event) => events.push(event));
    const mask = new CoverageMask(engine, "coverage mask");
    mask.update(merged(CoverageView.empty(8), cover([0, 10, 3]), 1));
    mask.update(merged(CoverageView.empty(8), cover([0, 20, 3]), 2));
    expect(events.filter((e) => e.kind === "created")).toEqual([
      { kind: "created", name: "coverage mask", bytes: 384 * 1024, category: "coarse-field-gpu" },
    ]);
    expect(engine.textureWritten).toHaveLength(2);
    expect(engine.textureWritten[1]?.data).toHaveLength(384 * 1024);
    mask.update(CoverageView.empty(5));
    mask.release();
    mask.release();
    expect(events.filter((e) => e.kind !== "uploaded").map((e) => [e.kind, e.bytes])).toEqual([
      ["created", 384 * 1024],
      ["destroyed", 384 * 1024],
      ["created", 6 * 32 * 32],
      ["destroyed", 6 * 32 * 32],
    ]);
    expect(mask.texture).toBeNull();
    expect(coverageMaskTextureSpec("m", 6)).toMatchObject({
      size: { width: 64, height: 64, depthOrArrayLayers: 6 },
      format: "r8unorm",
      category: "coarse-field-gpu",
    });
  });
});
