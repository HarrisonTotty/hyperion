// Runs `src/tools/atmosphereData.ts` through Vite's module runner, as `solarFactors.mjs` runs its
// tool (the repository's Node floor does not strip types unflagged). From `apps/hyperion`:
//   node scripts/atmosphereData.mjs matching --cie ../../crates/hyperion-fit/data/cie_cmf/CIE_xyz_1931_2deg.csv
//   node scripts/atmosphereData.mjs cross-sections --ozone <serdyuchenkogorshelev5digits.dat> --methane <ch4.txt>
// (each fetched by URL into a directory of its own, as the tool's header gives them).
// The tool's header names each input's source, version and checksum.
import { fileURLToPath } from "node:url";

import { runnerImport } from "vite";

const entry = fileURLToPath(new URL("../src/tools/atmosphereData.ts", import.meta.url));
const { module } = await runnerImport(entry, { configFile: false, logLevel: "error" });
process.exitCode = module.main(process.argv.slice(2));
