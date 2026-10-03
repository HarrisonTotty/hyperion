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

const REGISTER_VIEW = ["register", "View"].join("");
const TOLERANCE = ["toler", "ance"].join("");

const TOLERANCE_READ = new RegExp(String.raw`\.read(?:Buffer|Texture)\([^)]*["']${TOLERANCE}["']`);
const ENGINE_MODULE = ["webgpu", "/", "engine"].join("");
const STATIC_ENGINE_IMPORT = new RegExp(
  String.raw`(?:from\s+|import\s+)["'][^"']*${ENGINE_MODULE}["']`,
);
const RAW_DEVICE_ALLOCATION = /\b\w*[dD]evice\.create(?:Buffer|Texture)\(/;

/** Whether `path` lies in the adapter, the one directory that may allocate on the device. */
function inEngineAdapter(path: string): boolean {
  return path.startsWith("view/engine/webgpu/");
}

/** Whether `path` is part of the smoke harness's page. */
function inSmokePage(path: string): boolean {
  return path.startsWith("smoke/") || path.includes("/smoke/");
}

/**
 * Whether `path` is the descent spike's capture shim (R05.T15.a) or its test, the one module
 * outside the adapter that may allocate on the device: it forwards the engine's own
 * `createBuffer` and `createTexture` and makes the snapshot's staging buffers, for measurement
 * runs only, and its test drives a device directly to check that.
 */
function isCaptureShim(path: string): boolean {
  return path === "view/spike/capture.ts" || path === "view/spike/capture.test.ts";
}

/** Each rule `file` breaks, in words. */
function violations(file: SourceFile): string[] {
  const found: string[] = [];
  if (file.text.includes(REGISTER_VIEW)) {
    found.push(`${file.path} names ${REGISTER_VIEW}, whose drawImage copy the brainstorm rejects`);
  }
  if (!inSmokePage(file.path) && TOLERANCE_READ.test(file.text)) {
    found.push(`${file.path} reads back with ${TOLERANCE} access outside the smoke page`);
  }
  if (!inEngineAdapter(file.path) && STATIC_ENGINE_IMPORT.test(file.text)) {
    found.push(`${file.path} imports the engine module eagerly, not through loadEngine's import()`);
  } else if (
    !inEngineAdapter(file.path) &&
    file.path !== "view/engine/loadEngine.ts" &&
    file.text.includes(ENGINE_MODULE)
  ) {
    found.push(`${file.path} names the engine module, which only loadEngine.ts may load`);
  }
  if (
    !inEngineAdapter(file.path) &&
    !isCaptureShim(file.path) &&
    RAW_DEVICE_ALLOCATION.test(file.text)
  ) {
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

  it("refuses the view-registering copy anywhere", () => {
    const text = `engine.${REGISTER_VIEW}(canvas);`;
    expect(violations({ path: "view/engine/webgpu/engine.ts", text })).toHaveLength(1);
  });

  it("refuses a tolerance readback outside the smoke page", () => {
    const text = `await engine.readBuffer(sum, "${TOLERANCE}");`;
    expect(violations({ path: "view/scene.ts", text })).toHaveLength(1);
    expect(violations({ path: "smoke/kernels.ts", text })).toEqual([]);
  });

  it("refuses a static import of the engine module, even in the loader", () => {
    const text = `import { createWebGpuEngine } from "./${ENGINE_MODULE}";`;
    expect(violations({ path: "view/engine/loadEngine.ts", text })).toHaveLength(1);
  });

  it("lets only the loader import the engine module dynamically", () => {
    const text = `const engine = await import("./${ENGINE_MODULE}");`;
    expect(violations({ path: "view/engine/loadEngine.ts", text })).toEqual([]);
    expect(violations({ path: "view/scene.ts", text })).toHaveLength(1);
  });

  it("refuses a raw device allocation outside the adapter", () => {
    const text = "const buffer = device.createBuffer({ size: 4, usage: 1 });";
    expect(violations({ path: "view/scene.ts", text })).toHaveLength(1);
    expect(violations({ path: "view/engine/webgpu/resources.ts", text })).toEqual([]);
    const named = "this.gpuDevice.createTexture({ size: [1, 1], format: 'r8unorm', usage: 4 });";
    expect(violations({ path: "view/scene.ts", text: named })).toHaveLength(1);
  });

  it("exempts the spike's capture shim alone from the allocation rule", () => {
    const text = "const buffer = device.createBuffer({ size: 4, usage: 1 });";
    expect(violations({ path: "view/spike/capture.ts", text })).toEqual([]);
    expect(violations({ path: "view/spike/metrics.ts", text })).toHaveLength(1);
  });
});
