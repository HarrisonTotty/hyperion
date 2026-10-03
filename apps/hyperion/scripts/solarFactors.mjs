// Runs `src/tools/solarFactors.ts` through Vite's module runner, as `placeShip.mjs` runs its tool
// (the repository's Node floor does not strip types unflagged). Usage:
//   node scripts/solarFactors.mjs --e490 <e490_00a_amo.csv> --cie <CIE_xyz_1931_2deg.csv>
// The tool's header names each input's source, version and checksum.
import { fileURLToPath } from "node:url";

import { runnerImport } from "vite";

const entry = fileURLToPath(new URL("../src/tools/solarFactors.ts", import.meta.url));
const { module } = await runnerImport(entry, { configFile: false, logLevel: "error" });
process.exitCode = module.main(process.argv.slice(2));
