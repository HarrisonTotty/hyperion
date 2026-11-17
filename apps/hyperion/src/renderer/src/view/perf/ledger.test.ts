import {
  type AdapterRecord,
  isMemoryItem,
  MEMORY_ITEMS,
  MEMORY_ITEMS_ORDER,
  type PerfTimer,
} from "@hyperion/perf";
import { describe, expect, expectTypeOf, it } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { depthLayersTexture } from "../engine/depthLayers";
import { TEXTURE_USAGE } from "../engine/gpuFlags";
import { type MemoryCategory, type TextureSpec, textureBytes } from "../engine/memory";
import type { AdapterSummary } from "../engine/platform";
import type { GpuTimer } from "../engine/status";
import { mipSizes } from "../sky/mips";
import { HIGH_SKY, LOW_SKY } from "../sky/setting";
import { CASCADE_LAYERS } from "../terrain/shadows/terrainShadows";
import { allocationTally } from "../terrain/gpu/allocationTally";
import { itemise, itemOf, LEDGER_CATEGORIES, MemoryLedger } from "./ledger";

const MB = 1_000_000;
const MIB = 1024 ** 2;

/** R01's packed sky cube as the engine makes it (`webgpu/resources.ts`' `packedCubeSpec`). */
function skyCube(sizePx: number): TextureSpec {
  return {
    name: "sky cube",
    size: { width: sizePx, height: sizePx, depthOrArrayLayers: 6 },
    dimension: "cube",
    format: "rgb9e5ufloat",
    mips: mipSizes(sizePx).length,
    usage: TEXTURE_USAGE.TEXTURE_BINDING,
    category: "sky-cube",
  };
}

function target(format: GPUTextureFormat, widthPx: number, heightPx: number): TextureSpec {
  return {
    name: "target",
    size: [widthPx, heightPx],
    dimension: "2d",
    format,
    mips: 1,
    usage: TEXTURE_USAGE.RENDER_ATTACHMENT,
    category: "render-targets",
  };
}

describe("the memory items", () => {
  it("map exactly the renderer's categories (a type-level test over the union)", () => {
    expectTypeOf<keyof typeof MEMORY_ITEMS>().toEqualTypeOf<MemoryCategory>();
    expect(LEDGER_CATEGORIES).toHaveLength(Object.keys(MEMORY_ITEMS).length);
  });

  it("give every category a row of the memory table", () => {
    expect(LEDGER_CATEGORIES.filter((category) => !isMemoryItem(itemOf(category)))).toEqual([]);
    expect(itemOf("sky-cube")).toBe("star-cubemap");
    expect(itemOf("height-cache")).toBe("height-cache");
    expect(itemOf("coarse-field-gpu")).toBe("coarse-field-gpu");
    expect(itemOf("shadows")).toBe("shadows");
  });

  it("agree with R01's timer and adapter summary (type-level tests)", () => {
    expectTypeOf<GpuTimer>().toEqualTypeOf<PerfTimer>();
    expectTypeOf<AdapterSummary>().toEqualTypeOf<AdapterRecord>();
  });
});

describe("the sizes the ledger counts", () => {
  it("re-check the brainstorm's star cube: 34 MB with mips at 1,024², about 300 MB at 3,072²", () => {
    // 6 faces × 4 B (rgb9e5ufloat packs three channels in 32 bits) × side² × 4⁄3 for the mips.
    const low = textureBytes(skyCube(LOW_SKY.faceSizePx));
    const high = textureBytes(skyCube(HIGH_SKY.faceSizePx));
    expect(low / MB).toBeCloseTo((6 * 4 * 1024 ** 2 * 4) / 3 / MB, 3);
    expect(Math.round(low / (MB / 10)) / 10).toBe(33.6);
    expect(high / MB).toBeCloseTo((6 * 4 * 3072 ** 2 * 4) / 3 / MB, 2);
    expect(Math.round(high / MB)).toBe(302);
  });

  it("re-check the shadows' four 2,048² depth32float cascades as 64 MiB", () => {
    expect(CASCADE_LAYERS).toMatchObject({ sizePx: 2048, layers: 4 });
    expect(textureBytes(depthLayersTexture(CASCADE_LAYERS))).toBe(64 * MIB);
  });

  it("re-check the packed and half-float formats and the mip chain", () => {
    expect(textureBytes(target("rg11b10ufloat", 1920, 1080))).toBe(1920 * 1080 * 4);
    expect(textureBytes(target("rgb9e5ufloat", 1920, 1080))).toBe(1920 * 1080 * 4);
    expect(textureBytes(target("rgba16float", 1920, 1080))).toBe(1920 * 1080 * 8);
    // A full chain of a 256² rgba16float: 256² + 128² + … + 1², (4⁹ − 1) ⁄ 3 texels.
    expect(textureBytes({ ...target("rgba16float", 256, 256), mips: 9 })).toBe(
      ((4 ** 9 - 1) / 3) * 8,
    );
  });
});

describe("the ledger", () => {
  it("itemises the tally's live bytes by row", async () => {
    const engine = await countingRenderEngine();
    const tally = allocationTally(engine);
    engine.createBuffer({ name: "heights", bytes: 1000, usage: 0, category: "height-cache" });
    engine.createBuffer({ name: "pages", bytes: 300, usage: 0, category: "geometry-pages" });
    engine.createBuffer({ name: "detail", bytes: 200, usage: 0, category: "decoration" });
    const items = itemise(tally);
    expect(items["height-cache"]).toBe(1000);
    expect(items.other).toBe(500);
    expect(Object.keys(items)).toEqual([...MEMORY_ITEMS_ORDER]);
  });

  it("returns an item to zero on a release, and its peak never falls", async () => {
    const engine = await countingRenderEngine();
    const tally = allocationTally(engine);
    const ledger = new MemoryLedger(tally);
    engine.createTexture(skyCube(1024));
    const cubeBytes = textureBytes(skyCube(1024));
    expect(ledger.figures().liveBytes["star-cubemap"]).toBe(cubeBytes);
    engine.destroy("sky cube", cubeBytes, "sky-cube");
    const figures = ledger.figures();
    expect(figures.liveBytes["star-cubemap"]).toBe(0);
    expect(figures.peakBytes["star-cubemap"]).toBe(cubeBytes);
    expect(figures.tallyPeakBytes).toBe(cubeBytes);
    ledger.resetPeaks();
    expect(ledger.figures().peakBytes["star-cubemap"]).toBe(0);
    ledger.dispose();
  });

  it("takes an item's peak as its categories held together, not the sum of their peaks", async () => {
    const engine = await countingRenderEngine();
    const ledger = new MemoryLedger(allocationTally(engine));
    engine.createBuffer({ name: "detail", bytes: 100, usage: 0, category: "decoration" });
    engine.destroy("detail", 100, "decoration");
    engine.createBuffer({ name: "pages", bytes: 50, usage: 0, category: "geometry-pages" });
    const figures = ledger.figures();
    expect(figures.peakBytes.other).toBe(100);
    expect(figures.liveBytes.other).toBe(50);
    ledger.dispose();
  });
});

// --- The tally's call sites: every creation names its category. ---

/** Every renderer source file but the tests, keyed by its path from the app's root. */
const SOURCES: Readonly<Record<string, string>> = import.meta.glob<string>(
  ["/src/renderer/src/**/*.ts", "/src/renderer/src/**/*.tsx", "!/src/renderer/src/**/*.test.*"],
  { query: "?raw", import: "default", eager: true },
);

/** The engine's creation calls, assembled so that this file's own text never matches. */
const CREATION = new RegExp(String.raw`(\w*)\.(create(?:Buffer|Texture|PackedCube))\(`, "g");

/** The text from an opening bracket at `start` to its match, or `null` where it never closes. */
function balanced(text: string, start: number): string | null {
  const open = text[start];
  const close = open === "{" ? "}" : ")";
  let depth = 0;
  for (let i = start; i < text.length; i += 1) {
    const c = text[i];
    if (c === open) {
      depth += 1;
    } else if (c === close) {
      depth -= 1;
      if (depth === 0) {
        return text.slice(start, i + 1);
      }
    }
  }
  return null;
}

/** The top-level arguments of a call's bracketed text, `(a, f(b, c), d)` being three. */
function argumentCount(call: string): number {
  let depth = 0;
  let count = call.slice(1, -1).trim().length > 0 ? 1 : 0;
  for (const c of call.slice(1, -1)) {
    if ("([{".includes(c)) {
      depth += 1;
    } else if (")]}".includes(c)) {
      depth -= 1;
    } else if (c === "," && depth === 0) {
      count += 1;
    }
  }
  return count;
}

/** Each creation in `text` that names no category, in words. */
function uncategorised(path: string, text: string): string[] {
  const found: string[] = [];
  for (const match of text.matchAll(CREATION)) {
    const [, receiver = "", method = ""] = match;
    // A raw device's own creation takes a WebGPU descriptor: the engine boundary's test keeps it in
    // the adapter, behind the engine's creation path.
    if (/[dD]evice$/.test(receiver)) {
      continue;
    }
    const open = match.index + match[0].length - 1;
    const call = balanced(text, open);
    if (call === null) {
      continue;
    }
    const line = text.slice(0, open).split("\n").length;
    if (method === "createPackedCube") {
      // `createPackedCube(sizePx, mips, category, name?)`, and its declarations.
      if (argumentCount(call) < 3) {
        found.push(`${path}:${line} makes a packed cube without a category`);
      }
      continue;
    }
    const literal = call.trimStart().startsWith("({") ? balanced(call, call.indexOf("{")) : null;
    if (literal !== null && !/\bcategory\b/.test(literal) && !literal.includes("...")) {
      found.push(`${path}:${line} makes a ${method.slice(6).toLowerCase()} without a category`);
    }
  }
  return found;
}

describe("the tally's call sites", () => {
  it("are found across the renderer", () => {
    const sites = Object.values(SOURCES).reduce(
      (n, text) => n + [...text.matchAll(CREATION)].length,
      0,
    );
    expect(sites).toBeGreaterThan(100);
  });

  it("each name a category", () => {
    const found = Object.entries(SOURCES).flatMap(([path, text]) => uncategorised(path, text));
    expect(found).toEqual([]);
  });

  it("fail a creation without one", () => {
    const create = ["engine", "create"].join(".");
    expect(uncategorised("a.ts", `${create}Buffer({ name: "x", bytes: 4, usage: 0 });`)).toEqual([
      "a.ts:1 makes a buffer without a category",
    ]);
    expect(
      uncategorised(
        "a.ts",
        `${create}Texture({\n name: "t",\n size: [1, 1],\n format: "r8unorm" });`,
      ),
    ).toHaveLength(1);
    expect(uncategorised("a.ts", `${create}PackedCube(4, 2);`)).toHaveLength(1);
    expect(
      uncategorised(
        "a.ts",
        `${create}Buffer({ name: "x", bytes: 4, usage: 0, category: "other" });`,
      ),
    ).toEqual([]);
    expect(uncategorised("a.ts", `${create}Texture({ ...spec, name: "t" });`)).toEqual([]);
    expect(uncategorised("a.ts", `${create}PackedCube(4, 2, "sky-cube");`)).toEqual([]);
  });
});
