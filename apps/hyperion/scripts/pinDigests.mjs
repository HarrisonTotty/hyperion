// Runs `src/tools/pinDigests.ts` (`just pin-digests`) through Vite's module runner, as
// `placeShip.mjs` runs its tool. `--check` compares the pins and writes nothing.
import { fileURLToPath } from "node:url";

import { runnerImport } from "vite";

const entry = fileURLToPath(new URL("../src/tools/pinDigests.ts", import.meta.url));
const { module } = await runnerImport(entry, { configFile: false, logLevel: "error" });
process.exitCode = await module.main(process.argv.slice(2));
