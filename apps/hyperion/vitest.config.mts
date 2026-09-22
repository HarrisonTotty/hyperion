import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

import pkg from "./package.json" with { type: "json" };

// The tests that need a DOM: every component test, and the three logic tests that render a hook or
// paint a real canvas. Everything else is pure functions and runs in Node, which costs nothing to
// create — a jsdom for every file was 45% of the suite's tracked time (measured 2026-09-22 on
// 8 cores: 74.9 s across 56 files), and the half that needs no DOM now runs in about 1.3 s. A new
// `.test.ts` that reaches for `document` or `renderHook` fails with "document is not defined";
// list it here, or name it `.test.tsx` if it carries JSX.
const DOM_TESTS = [
  "src/**/*.test.tsx",
  "src/renderer/src/lib/connection.test.ts",
  "src/renderer/src/spatial/paint.test.ts",
  "src/renderer/src/spatial/useThrottledValue.test.ts",
];

export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  test: {
    restoreMocks: true,
    unstubGlobals: true,
    // A display test drives the whole console through `user-event`, and the heaviest of them (the
    // local chart's, and the end-to-end script of P05.T12.a) take about 2 s each with the suite's
    // workers on eight cores. Vitest's default of 5 s left them failing whenever the machine was
    // also building something else, which is a flake and not a finding, so the limit is what a
    // test that is genuinely stuck needs to cross.
    testTimeout: 15_000,
    // Workers are reused across files instead of one being spawned per file, which vitest costed
    // at about 2.9 s of spawn and environment each. With the split above, the suite goes from
    // about 30 s to about 14 s. It gives up the isolation of the module registry and of the jsdom,
    // so the suite has to be order-independent, which is a rule it already keeps:
    // `vitest run --sequence.shuffle` passes. The shared setup clears what would otherwise carry
    // from one file to the next.
    isolate: false,
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
