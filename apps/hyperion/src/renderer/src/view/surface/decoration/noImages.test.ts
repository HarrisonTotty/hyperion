import { describe, expect, it } from "vitest";

/**
 * No texture image is authored or shipped (plan R11, R11.T2.c; Design note 21, the owner's ruling
 * of 2026-11-01): every texture the view samples is generated for the world it is drawn on. Checked
 * by reading every source under `view/`, as `wasm/surfaceImports.test.ts` checks its boundary, and
 * by listing every file there. The image extensions are assembled at run time, so that this file's
 * own text never matches the rule.
 */

/** The image formats a texture could be authored in. */
const IMAGE_EXTENSIONS = [
  "png",
  "jpg",
  "jpeg",
  "gif",
  "webp",
  "avif",
  "bmp",
  "tga",
  "dds",
  "ktx",
  "ktx2",
  "basis",
  "exr",
  "hdr",
  "svg",
];

/** Every `.ts`, `.tsx` and `.wgsl` file under `view/`, keyed by its path from the app's root. */
const SOURCES: Readonly<Record<string, string>> = import.meta.glob<string>(
  [
    "/src/renderer/src/view/**/*.ts",
    "/src/renderer/src/view/**/*.tsx",
    "/src/renderer/src/view/**/*.wgsl",
  ],
  { query: "?raw", import: "default", eager: true },
);

/** Every file under `view/`, by path, that an image format names: none may exist. */
const IMAGE_FILES: ReadonlyArray<string> = Object.keys(
  import.meta.glob(
    [
      "/src/renderer/src/view/**/*.{png,jpg,jpeg,gif,webp,avif,bmp,tga,dds,ktx,ktx2,basis,exr,hdr,svg}",
    ],
    { query: "?url", import: "default" },
  ),
);

/** A specifier, in an import or a `new URL`, of an image file. */
const IMAGE_SPECIFIER = new RegExp(
  String.raw`(?:from\s+|import\s*\(\s*|import\s+|new\s+URL\s*\(\s*)["'][^"']*\.(?:${IMAGE_EXTENSIONS.join("|")})(?:\?[^"']*)?["']`,
  "i",
);

/** An image's decoding into a texture's source at run time. */
const IMAGE_DECODE = new RegExp(String.raw`\bnew\s+${"Image"}\s*\(|\bcreate${"ImageBitmap"}\s*\(`);

/** The rule `text` breaks, if any. */
function violation(path: string, text: string): string | null {
  if (IMAGE_SPECIFIER.test(text)) {
    return `${path} imports an image file`;
  }
  if (IMAGE_DECODE.test(text)) {
    return `${path} decodes an image`;
  }
  return null;
}

describe("the view's textures (Design note 21)", () => {
  it("reads the view's sources", () => {
    expect(Object.keys(SOURCES).length).toBeGreaterThan(100);
    expect(Object.keys(SOURCES).some((path) => path.endsWith(".wgsl"))).toBe(true);
  });

  it("imports no image file and decodes none", () => {
    const broken = Object.entries(SOURCES).flatMap(([path, text]) => {
      if (path.endsWith(".test.ts") || path.endsWith(".test.tsx")) {
        return [];
      }
      const found = violation(path, text);
      return found === null ? [] : [found];
    });
    expect(broken).toEqual([]);
  });

  it("holds no image file", () => {
    expect(IMAGE_FILES).toEqual([]);
  });

  it("catches an image import, a URL and a decode", () => {
    const png = ["sand", "png"].join(".");
    expect(violation("a.ts", `import sand from "./${png}";`)).not.toBeNull();
    expect(violation("a.ts", `const u = new URL("./${png}?url", import.meta.url);`)).not.toBeNull();
    expect(violation("a.ts", "const i = new Image();")).not.toBeNull();
    expect(violation("a.ts", 'import { x } from "./decoration";')).toBeNull();
  });
});
