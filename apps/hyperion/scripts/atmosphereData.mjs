// Runs `src/tools/atmosphereData.ts` through Vite's module runner, as `solarFactors.mjs` runs its
// tool (Node strips the types, but the tool's modules import each other without extensions, which
// only Vite resolves). From `apps/hyperion`:
//   node scripts/atmosphereData.mjs matching --cie ../../crates/hyperion-fit/data/cie_cmf/CIE_xyz_1931_2deg.csv
//   node scripts/atmosphereData.mjs cross-sections --ozone <serdyuchenkogorshelev5digits.dat> --methane <ch4.txt> \
//     --so2 <298 K file> --so2 <318 K> --so2 <338 K> --so2 <358 K> --o2o2 <O2-O2_2024.cia> \
//     --h2he <H2-He_2011.cia> --o2n2 <O2-N2_2024.cia> --o2co2 <O2-CO2_2024.cia> \
//     --h2h2 <final_CIA_LT.dat> --h2h2 <final_CIA_HT.dat> --h2h2 <CIA.H2H2.Yi>
// (each fetched by URL into a directory of its own, as the tool's header gives them; the SO2 files
// are extracted from the atlas's snapshot archive).
// The tool's header names each input's source, version and checksum. ExoMol's temperate lines
// (R08.T4.g) take `exomol-bands` (N2O), `exomol-bins`, `exomol-k-distributions` and
// `exomol-k-distributions-merge`, as `src/tools/exomolCli.ts` gives them; they stream the lists'
// `.bz2` files through the system's `bzip2 -dc`, which must be on the PATH. The atoms' and ions'
// tables (R08.T4.e) take `atomic-floors`, `atomic-projection`, `atomic-reprojection`,
// `atomic-k-distributions`, `atomic-k-distributions-merge`, `atomic-lines`,
// `atomic-cell-references` and `alkali-pairs`, as `src/tools/atomicKDistributionsCli.ts` gives
// them.
import { fileURLToPath } from "node:url";

import { runnerImport } from "vite";

const entry = fileURLToPath(new URL("../src/tools/atmosphereData.ts", import.meta.url));
const { module } = await runnerImport(entry, { configFile: false, logLevel: "error" });
// `exomol-bins` streams its inputs and returns a promise of its exit code; the others return it.
process.exitCode = await module.main(process.argv.slice(2));
