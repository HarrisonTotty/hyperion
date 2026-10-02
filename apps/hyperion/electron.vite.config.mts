import { resolve } from "node:path";

import react from "@vitejs/plugin-react";
import { defineConfig } from "electron-vite";

import pkg from "./package.json" with { type: "json" };

// The headless smoke harness (R01.T9) is a second entry of each build: its own main process,
// preload and page, beside the client's. Neither the client's main process nor its page imports it.
export default defineConfig({
  main: {
    build: {
      rolldownOptions: {
        input: { index: resolve("src/main/index.ts"), smoke: resolve("src/smoke/main.ts") },
      },
    },
  },
  preload: {
    build: {
      rolldownOptions: {
        input: { index: resolve("src/preload/index.ts"), smoke: resolve("src/smoke/preload.ts") },
      },
    },
  },
  renderer: {
    plugins: [react()],
    define: {
      __APP_VERSION__: JSON.stringify(pkg.version),
    },
    build: {
      rolldownOptions: {
        input: {
          index: resolve("src/renderer/index.html"),
          smoke: resolve("src/renderer/smoke.html"),
        },
      },
    },
    // Workers are ES modules in the build as on the dev server, so that a module worker such as
    // `wasm/surface.worker.ts` loads the same way in both (R04 Design note 16).
    worker: {
      format: "es",
    },
  },
});
