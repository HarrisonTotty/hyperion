import { expect, it } from "vitest";

import config from "../../electron.vite.config.mjs";
import pkg from "../../package.json" with { type: "json" };

// `app.getVersion()` gave Electron's version on the spike's launch from `out/main/index.js`, so a
// run recorded 44.4.3 as the app's (R05.T14.c's hidden run).
it("builds the main process with the app's own version, which a spike run records", () => {
  expect(config.main?.define?.["__APP_VERSION__"]).toBe(JSON.stringify(pkg.version));
});
