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
  },
});
