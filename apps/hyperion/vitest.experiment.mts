// Scratch config for profiling: two projects, node for pure-logic tests and jsdom for DOM tests.
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

import pkg from "./package.json" with { type: "json" };

const domOnly = [
  "src/**/*.test.tsx",
  "src/renderer/src/lib/connection.test.ts",
  "src/renderer/src/spatial/useThrottledValue.test.ts",
  "src/renderer/src/spatial/paint.test.ts",
];

export default defineConfig({
  plugins: [react()],
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  test: {
    restoreMocks: true,
    unstubGlobals: true,
    projects: [
      {
        extends: true,
        test: {
          name: "logic",
          environment: "node",
          include: ["src/**/*.test.ts"],
          exclude: domOnly,
        },
      },
      {
        extends: true,
        test: {
          name: "dom",
          environment: "jsdom",
          include: domOnly,
          setupFiles: ["src/renderer/src/test/setup.ts"],
        },
      },
    ],
  },
});
