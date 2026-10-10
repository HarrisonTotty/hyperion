// Runs `src/tools/materials.ts` through Vite's module runner, as `solarFactors.mjs` runs its tool
// (the repository's Node floor does not strip types unflagged). The written files are then
// formatted with the repository's Prettier, the last command below. Usage, every source fetched as
// the tool's header names it:
//   node scripts/materials.mjs --out src/renderer/src/view/atmosphere/materials \
//     --water <Hale.yml> --ice <Warren-2008.yml> --ammonia <nh3-m-Martonchik1983.lnk> \
//     --methane-liquid <Martonchik-liquid-90K.yml> --methane-ice <Martonchik-solid-90K.yml> \
//     --h2so4-75 <H2SO4_75%_300K_R_Palmer_1975.ri> --h2so4-84 <H2SO4_84.5%_300K_R_Palmer_1975.ri> \
//     --iron <Johnson.yml> --soot <Dalzell.yml> --tholin <khare_tholins.dat> \
//     --mars-dust <Dust_Refractive_Indicies.txt> --enstatite <pyr-mg100-Dorschner1995.lnk> \
//     --forsterite <jager_mg2sio4.dat>
//   pnpm exec prettier --write src/renderer/src/view/atmosphere/materials/*.json
import { fileURLToPath } from "node:url";

import { runnerImport } from "vite";

const entry = fileURLToPath(new URL("../src/tools/materials.ts", import.meta.url));
const { module } = await runnerImport(entry, { configFile: false, logLevel: "error" });
process.exitCode = module.main(process.argv.slice(2));
