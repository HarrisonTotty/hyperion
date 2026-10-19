// Runs `src/tools/materials.ts` through Vite's module runner, as `solarFactors.mjs` runs its tool
// (chosen while the repository's Node floor, 22.12, stripped no types unflagged). The written files
// are then formatted with the repository's Prettier, the last command below. Usage, every source
// fetched as the tool's header names it:
//   node scripts/materials.mjs --out src/renderer/src/view/atmosphere/materials \
//     --ice <Warren-2008.yml> --ammonia <nh3-m-Martonchik1983.lnk> \
//     --methane-liquid <Martonchik-liquid-90K.yml> --methane-ice <Martonchik-solid-90K.yml> \
//     --h2so4-75 <H2SO4_75%_300K_R_Palmer_1975.ri> --h2so4-84 <H2SO4_84.5%_300K_R_Palmer_1975.ri> \
//     --iron <Johnson.yml> --soot <Dalzell.yml> --tholin <khare_tholins.dat> \
//     --mars-dust <Dust_Refractive_Indicies.txt> --enstatite <pyr-mg100-Dorschner1995.lnk> \
//     --forsterite <jager_mg2sio4.dat> \
//     --chromium <Cr/nk/Johnson.yml> --iron-sulphide <FeS.dat> --calcium-titanate <CaTiO3.dat> \
//     --zinc-sulphide <ZnS.dat> --manganese-sulphide <MnS.dat> --sodium-sulphide <Na2S.dat> \
//     --titanium-dioxide <TiO2/nk/Siefke.yml> --argon <Ar/nk/Sinnock-solid-80K.yml> \
//     --chromophore <jupiter_muse_paper4.tex> --ice-giant-haze <Neptune_Uranus_Haze.tex> \
//     --basalt <basalt_R_Pollack_1973.ri> --basaltic-glass <basaltic_glass_R_Pollack_1973.ri> \
//     --mgs1 <apjsacf0bet6_ascii.txt> --potassium-chloride <KCl/nk/Li.yml> \
//     --sodium-chloride <NaCl/nk/Li.yml> --silicon-dioxide-ordinary <SiO2/nk/Ghosh-o.yml> \
//     --silicon-dioxide-extraordinary <SiO2/nk/Ghosh-e.yml> \
//     --aluminium-oxide-ordinary <Al2O3/nk/Malitson-o.yml> \
//     --aluminium-oxide-extraordinary <Al2O3/nk/Malitson-e.yml> \
//     --graphite-perpendicular <callindex.out_CpeD03_0.01> \
//     --graphite-parallel <callindex.out_CpaD03_0.01> --pope-fry <pope97.txt> \
//     --mason <purewater_abs_coefficients_v3.txt> \
//     --argon-liquid <Ar/nk/Sinnock-liquid-83.81K.yml> \
//     --krypton-liquid <Kr/nk/Sinnock-liquid-115.95K.yml> \
//     --xenon-liquid <Xe/nk/Sinnock-liquid-161.35K.yml> --o2o2 <O2-O2_2024.cia> \
//     --karkoschka-1998 <1995LOW.TAB>
//   pnpm exec prettier --write src/renderer/src/view/atmosphere/materials/*.json
// The two .tex files are the e-prints' members, extracted from their archives as data; the .dat
// files are Kitzmann and Heng 2018's compilation's (R08.T5.d). `pope97.txt` is omlc.org's table of
// Pope and Fry 1997 (R08.T5.g), and `purewater_abs_coefficients_v3.txt` WOPP v3's, which holds Mason,
// Cone and Fry 2016's table (R08.T5.h): the water and seawater files take k from both. Since R08.T5.h
// no file reads Hale and Querry's table. Since R08.T5.k the three `Sinnock-liquid-*.yml` files,
// refractiveindex.info's copies of Sinnock and Smith 1969's liquid tables, make argon-liquid.json,
// krypton-liquid.json and xenon-liquid.json, and `O2-O2_2024.cia`, HITRAN2024's collision-induced
// absorption file (the atmosphere tool's `--o2o2`), gives oxygen-liquid.json its k. Since R08.T5.j
// `1995LOW.TAB`, Karkoschka 1998's methane coefficients from NASA's Planetary Data System (volume
// GBAT_0001), gives methane-liquid.json its k over 380–555.6 nm, with Martonchik-liquid-90K.yml's n.
import { fileURLToPath } from "node:url";

import { runnerImport } from "vite";

const entry = fileURLToPath(new URL("../src/tools/materials.ts", import.meta.url));
const { module } = await runnerImport(entry, { configFile: false, logLevel: "error" });
process.exitCode = module.main(process.argv.slice(2));
