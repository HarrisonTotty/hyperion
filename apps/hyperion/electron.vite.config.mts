import react from "@vitejs/plugin-react";
import { defineConfig } from "electron-vite";

import pkg from "./package.json" with { type: "json" };

export default defineConfig({
  main: {},
  preload: {},
  renderer: {
    plugins: [react()],
    define: {
      __APP_VERSION__: JSON.stringify(pkg.version),
    },
    // Workers are ES modules in the build as on the dev server, so that a module worker such as
    // `wasm/surface.worker.ts` loads the same way in both (R04 Design note 16).
    worker: {
      format: "es",
    },
    build: {
      rolldownOptions: {
        output: {
          // The engine is loaded lazily, by `view/engine/loadEngine.ts`'s dynamic import, into a
          // chunk of its own named `babylon`, which `scripts/checkChunks.mjs` checks for. Rolldown
          // ignores `manualChunks` once `codeSplitting` is set, and in Vite 8 `rollupOptions` is a
          // deprecated alias of `rolldownOptions` (R01 Design note 14). Vite's preload helper,
          // shared by the entry and every dynamic import, gets a group of its own: without it,
          // rolldown put it in the `babylon` chunk, which the entry then imported eagerly.
          codeSplitting: {
            groups: [
              { name: "babylon", test: /node_modules[\\/]@babylonjs/ },
              { name: "preload-helper", test: /vite[\\/]preload-helper/, priority: 1 },
            ],
          },
        },
      },
    },
  },
});
