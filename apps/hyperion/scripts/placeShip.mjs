// Runs `src/tools/placeShip.ts` (`just place-ship`) through Vite's module runner, which compiles
// the TypeScript and resolves `@hyperion/protocol`'s sources as the client's build does, so the
// tool needs no build step of its own. Arguments after the script reach the tool.
import { fileURLToPath } from "node:url";

import { runnerImport } from "vite";

const entry = fileURLToPath(new URL("../src/tools/placeShip.ts", import.meta.url));
const { module } = await runnerImport(entry, { configFile: false, logLevel: "error" });
process.exitCode = await module.main(process.argv.slice(2));
