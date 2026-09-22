// Scratch config for profiling: variant B, logic project without the React plugin.
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

import pkg from "./package.json" with { type: "json" };

const domOnly = [
  "src/**/*.test.tsx",
  "src/renderer/src/lib/connection.test.ts",
  "src/renderer/src/spatial/useThrottledValue.test.ts",
  "src/renderer/src/spatial/paint.test.ts",
];

const define = { __APP_VERSION__: JSON.stringify(pkg.version) };

export default defineConfig({
  test: {
    projects: [
      {
        define,
        test: {
          name: "logic",
          environment: "node",
          include: ["src/**/*.test.ts"],
          exclude: domOnly,
          restoreMocks: true,
          unstubGlobals: true,
        },
      },
      {
        plugins: [react()],
        define,
        test: {
          name: "dom",
          environment: "jsdom",
          include: domOnly,
          setupFiles: ["src/renderer/src/test/setup.ts"],
          restoreMocks: true,
          unstubGlobals: true,
        },
      },
    ],
  },
});
