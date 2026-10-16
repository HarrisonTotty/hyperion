import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

import pkg from "./package.json" with { type: "json" };

// The tests that need a DOM: every component test, and the logic tests that render a hook, paint a
// real canvas, test key presses against DOM elements or hand an engine a canvas. Everything else is
// pure functions and runs in Node, which costs nothing to create — a jsdom for every file was 45% of
// the suite's tracked time (measured 2026-09-22 on 8 cores: 74.9 s across 56 files), and the half
// that needs no DOM now runs in about 1.3 s. A new
// `.test.ts` that reaches for `document` or `renderHook` fails with "document is not defined";
// list it here, or name it `.test.tsx` if it carries JSX.
const DOM_TESTS = [
  "src/**/*.test.tsx",
  "src/renderer/src/displays/view/check/viewsProbe.test.ts",
  "src/renderer/src/lib/connection.test.ts",
  "src/renderer/src/lib/strokes.test.ts",
  "src/renderer/src/smoke/spatial.test.ts",
  "src/renderer/src/spatial/paint.test.ts",
  "src/renderer/src/spatial/useThrottledValue.test.ts",
  "src/renderer/src/view/camera/keys.test.ts",
  "src/renderer/src/view/engine/resilientEngine.test.ts",
  "src/renderer/src/view/spike/spikeRun.test.ts",
  "src/renderer/src/view/terrain/coarseFieldStore.test.ts",
  "src/renderer/src/wasm/useSurfaceModuleCheck.test.ts",
];

// A bless (`HYPERION_BLESS=1`, the Rust testkit's variable) turns snapshot updates on, so that a
// bless-style test's `toMatchFileSnapshot` rewrites its committed table
// (`src/renderer/src/test/bless.ts`, plan R08.T4.a). Never under `CI`, as golden.rs refuses: there
// the helper fails instead. Otherwise nothing is written, a missing file included, so a committed
// table is only ever compared.
const BLESSING = process.env["HYPERION_BLESS"] === "1" && process.env["CI"] === undefined;

export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  // The surface module's test reads the built `.wasm` as an inlined asset (`?inline`), and the
  // line absorbers' tests their binary k-distribution parts (R08.T4.d), since renderer code, tests
  // included, reads no files through `node:fs` (R04.T10.c).
  assetsInclude: ["**/*.wasm", "**/*.bin"],
  test: {
    update: BLESSING ? "all" : "none",
    restoreMocks: true,
    unstubGlobals: true,
    // A display test drives the whole console through `user-event`, and the heaviest of them (the
    // local chart's, and the end-to-end script of P05.T12.a) take up to 1.1 s and 1.2 s each in
    // the suite's run at 4 workers on eight threads, and 2.1 s with a second run beside it
    // (2026-10-15). Vitest's default of 5 s left them failing whenever the machine was also
    // building something else, which is a flake and not a finding, so the limit is what a test
    // that is genuinely stuck needs to cross. A loaded desktop has stalled one of them 70 times
    // over, so the tests a load can carry past this, and the hooks it can carry past vitest's 10 s
    // hook limit, set their own. Each such limit's comment gives its worst time in that run (M2)
    // and with the second beside it (M3), the desktop's, and the margin: 100 times M2, or 10 times
    // for CPU-bound work, which the desktop's loads did not stall, and over twice the desktop's,
    // rounded up to 30, 60, 120 or 300 s.
    testTimeout: 15_000,
    // Workers are reused across files instead of one being spawned per file, which vitest costed
    // at about 2.9 s of spawn and environment each. With the split above, the suite goes from
    // about 30 s to about 14 s. It gives up the isolation of the module registry and of the jsdom,
    // so the suite has to be order-independent, which is a rule it already keeps:
    // `vitest run --sequence.shuffle` passes. The shared setup clears what would otherwise carry
    // from one file to the next.
    isolate: false,
    // A stylesheet imported for its effect is empty in a test, as Vitest makes every CSS file, so
    // that jsdom applies no layout rules; one imported as text (`?raw`) keeps its text, so that a
    // test can read a rule the guide requires, such as a plate's `--surface-0` (R07.T16.a).
    css: { include: [/\.css\?raw$/u] },
    projects: [
      {
        extends: true,
        test: {
          name: "logic",
          environment: "node",
          include: ["src/**/*.test.ts"],
          exclude: DOM_TESTS,
        },
      },
      {
        // The React plugin's JSX transform is only worth paying for on the files that have JSX.
        extends: true,
        plugins: [react()],
        test: {
          name: "dom",
          environment: "jsdom",
          include: DOM_TESTS,
          setupFiles: ["src/renderer/src/test/setup.ts"],
        },
      },
    ],
  },
});
