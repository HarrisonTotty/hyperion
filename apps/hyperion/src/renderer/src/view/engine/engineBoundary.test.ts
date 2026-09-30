import { describe, expect, it } from "vitest";

/**
 * The boundary around the engine, checked by reading every renderer source file (R01 Design note
 * 1): a test rather than a lint override, since the TypeScript rules forbid changing a rule's scope
 * in `.oxlintrc.json` without asking. The rules are a textual tripwire, matching how a call is
 * spelled rather than following values: a `"tolerance"` held in a variable passes them. The fixture strings are assembled at run time, so that this
 * file's own text never matches its rules, and the file skips itself all the same.
 */

/** Where the renderer's sources lie, from the app's root, which is Vite's. */
const ROOT = "/src/renderer/src/";

/** Every `.ts` and `.tsx` file under `src/renderer/src`, keyed by its path from the app's root. */
const SOURCES: Readonly<Record<string, string>> = import.meta.glob<string>(
  ["/src/renderer/src/**/*.ts", "/src/renderer/src/**/*.tsx"],
  { query: "?raw", import: "default", eager: true },
);

const SELF = `${ROOT}view/engine/engineBoundary.test.ts`;

interface SourceFile {
  /** The path from `src/renderer/src`, such as `view/engine/platform.ts`. */
  readonly path: string;
  readonly text: string;
}

const ENGINE_PACKAGE = ["@babylonjs", "/"].join("");
const REGISTER_VIEW = ["register", "View"].join("");
const TOLERANCE = ["toler", "ance"].join("");

const ENGINE_IMPORT = new RegExp(
  String.raw`(?:from\s+|import\s*\(\s*|import\s+)["']${ENGINE_PACKAGE}`,
);
const TOLERANCE_READ = new RegExp(String.raw`\.read(?:Buffer|Texture)\([^)]*["']${TOLERANCE}["']`);
const RAW_DEVICE_ALLOCATION = /\b\w*[dD]evice\.create(?:Buffer|Texture)\(/;

/** Whether `path` lies in the one directory that may import the engine. */
function inEngineAdapter(path: string): boolean {
  return path.startsWith("view/engine/babylon/");
}

/** Whether `path` is part of the smoke harness's page. */
function inSmokePage(path: string): boolean {
  return path.startsWith("smoke/") || path.includes("/smoke/");
}

/** Each rule `file` breaks, in words. */
function violations(file: SourceFile): string[] {
  const found: string[] = [];
  if (!inEngineAdapter(file.path) && ENGINE_IMPORT.test(file.text)) {
    found.push(`${file.path} imports the engine outside view/engine/babylon/`);
  }
  if (file.text.includes(REGISTER_VIEW)) {
    found.push(`${file.path} names ${REGISTER_VIEW}, whose drawImage copy the brainstorm rejects`);
  }
  if (!inSmokePage(file.path) && TOLERANCE_READ.test(file.text)) {
    found.push(`${file.path} reads back with ${TOLERANCE} access outside the smoke page`);
  }
  if (!inEngineAdapter(file.path) && RAW_DEVICE_ALLOCATION.test(file.text)) {
    found.push(`${file.path} allocates GPU memory outside the engine's one creation path`);
  }
  return found;
}

function sourceFiles(): SourceFile[] {
  return Object.entries(SOURCES)
    .filter(([key]) => key !== SELF)
    .map(([key, text]) => ({ path: key.slice(ROOT.length), text }));
}

describe("the engine boundary", () => {
  it("finds every renderer source file but its own", () => {
    const paths = sourceFiles().map(({ path }) => path);
    expect(paths).toContain("App.tsx");
    expect(paths).toContain("view/engine/platform.ts");
    expect(paths).not.toContain("view/engine/engineBoundary.test.ts");
  });

  it("holds on the tree", () => {
    expect(sourceFiles().flatMap(violations)).toEqual([]);
  });

  it("refuses an engine import outside the adapter", () => {
    const text = `import { Engine } from "${ENGINE_PACKAGE}core";`;
    expect(violations({ path: "view/scene.ts", text })).toHaveLength(1);
    expect(violations({ path: "view/engine/babylon/engine.ts", text })).toEqual([]);
  });

  it("refuses a dynamic engine import outside the adapter", () => {
    const text = `const core = await import("${ENGINE_PACKAGE}core");`;
    expect(violations({ path: "view/scene.ts", text })).toHaveLength(1);
  });

  it("refuses the view-registering copy anywhere", () => {
    const text = `engine.${REGISTER_VIEW}(canvas);`;
    expect(violations({ path: "view/engine/babylon/engine.ts", text })).toHaveLength(1);
  });

  it("refuses a tolerance readback outside the smoke page", () => {
    const text = `await engine.readBuffer(sum, "${TOLERANCE}");`;
    expect(violations({ path: "view/scene.ts", text })).toHaveLength(1);
    expect(violations({ path: "smoke/kernels.ts", text })).toEqual([]);
  });

  it("refuses a raw device allocation outside the adapter", () => {
    const text = "const buffer = device.createBuffer({ size: 4, usage: 1 });";
    expect(violations({ path: "view/scene.ts", text })).toHaveLength(1);
    expect(violations({ path: "view/engine/babylon/resources.ts", text })).toEqual([]);
    const named = "this.gpuDevice.createTexture({ size: [1, 1], format: 'r8unorm', usage: 4 });";
    expect(violations({ path: "view/scene.ts", text: named })).toHaveLength(1);
  });
});
