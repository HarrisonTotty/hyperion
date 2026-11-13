// Runs `src/tools/nonSpherical.ts` through Vite's module runner, as `materials.mjs` runs its tool.
// The archives are fetched, checked by MD5 and unpacked by hand into their own directories, as
// the tool's header names them, and deleted after the run. Usage:
//   node scripts/nonSpherical.mjs --materials src/renderer/src/view/atmosphere/materials \
//     --out src/renderer/src/view/atmosphere/materials/phase \
//     --tamudust <unpacked>/tables --angles <TAMUdust2020-v1.0/examples/params/TAMUdust2020_Angle> \
//     --excerpt src/renderer/src/view/atmosphere/fixtures/tamudust-excerpt.json
//   node scripts/nonSpherical.mjs --materials … --out … --yang <unpacked>/8_columns/Rough050
//   pnpm exec prettier --write src/renderer/src/view/atmosphere/materials/phase/*.json
import { fileURLToPath } from "node:url";

import { runnerImport } from "vite";

const entry = fileURLToPath(new URL("../src/tools/nonSpherical.ts", import.meta.url));
const { module } = await runnerImport(entry, { configFile: false, logLevel: "error" });
process.exitCode = await module.main(process.argv.slice(2));
