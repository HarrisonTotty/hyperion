// Runs `src/tools/atmosphereData.ts` through Vite's module runner, as `solarFactors.mjs` runs its
// tool (the repository's Node floor does not strip types unflagged). From `apps/hyperion`:
//   node scripts/atmosphereData.mjs matching --cie ../../crates/hyperion-fit/data/cie_cmf/CIE_xyz_1931_2deg.csv
//   node scripts/atmosphereData.mjs cross-sections --ozone <serdyuchenkogorshelev5digits.dat> --methane <ch4.txt> \
//     --so2 <298 K file> --so2 <318 K> --so2 <338 K> --so2 <358 K> --o2o2 <O2-O2_2024.cia> \
//     --h2he <H2-He_2011.cia> --o2n2 <O2-N2_2024.cia> --o2co2 <O2-CO2_2024.cia> \
//     --h2h2 <final_CIA_LT.dat> --h2h2 <final_CIA_HT.dat> --h2h2 <CIA.H2H2.Yi>
// (each fetched by URL into a directory of its own, as the tool's header gives them; the SO2 files
// are extracted from the atlas's snapshot archive).
// The tool's header names each input's source, version and checksum.
import { fileURLToPath } from "node:url";

import { runnerImport } from "vite";

const entry = fileURLToPath(new URL("../src/tools/atmosphereData.ts", import.meta.url));
const { module } = await runnerImport(entry, { configFile: false, logLevel: "error" });
process.exitCode = module.main(process.argv.slice(2));
