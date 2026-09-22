import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

import pkg from "./package.json" with { type: "json" };

// The tests that need a DOM: every component test, and the three logic tests that render a hook or
// paint a real canvas. Everything else is pure functions and runs in Node, which costs nothing to
// create — building a jsdom for all 49 files was half of the suite's time (measured 2026-09-22:
// 114 s of 250 s tracked). A new `.test.ts` that reaches for `document` or `renderHook` fails with
// "document is not defined"; list it here, or name it `.test.tsx` if it carries JSX.
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
    // Workers are reused across files instead of one being spawned per file, which halves the
    // suite again (about 26 s to about 13 s on 8 cores). It costs the isolation of the module
    // registry and of the jsdom, so the suite has to be order-independent, which is a rule it
    // already keeps: `vitest run --sequence.shuffle` passes. The shared setup restores anything
    // that would otherwise carry from one file to the next.
    // isolate on
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
