/**
 * The Balmer lines' Stark half-widths (plan R08, R08.T4.e; the brief's §3.1, "Stark broadening for
 * Balmer"), from C. Stehlé and R. Hutcheon, "Extensive tabulations of Stark broadened hydrogen line
 * profiles", Astron. Astrophys. Suppl. Ser. 140 (1999) 93 (CDS VI/98A; A&AS origin, free for a
 * scientific usage, CDS's terms read 2026-10-19), their Model Microfield Method for the electrons
 * and the ions of a pure hydrogen plasma.
 *
 * @remarks
 * Each profile file's header (the catalogue's ReadMe, "Description of files", lines 1–9) gives the
 * line's levels and wavelength, the electron density, the factor f0 from the reduced detuning α to
 * Å, and at each of its temperatures the profile's half width at half maximum without Doppler
 * broadening, in units of α. The lines here are H3–H10 (Hα to 379.9 nm), the Balmer lines whose
 * 25 cm⁻¹ shape reaches a cell of 380–760 nm, at the catalogue's first eleven densities, 10¹⁰ to
 * 10¹⁵ cm⁻³ every half decade, and its first three temperatures, 2,500, 5,000 and 10,000 K. Only
 * the half widths are taken, a reduced value; the profiles are not committed.
 *
 * The tables' line shape is Voigt, so the Stark profile enters as a Lorentzian of its half width.
 * That is an approximation: the quasi-static ion wing falls as the detuning to the −5/2, so beyond
 * a few half widths the Lorentzian overstates it, and H I's table is `estimated`.
 */

import { join } from "node:path";

import { readCheckedText } from "./molecularLines";

/** The catalogue's Balmer directory (CDS VI/98A), each file at `<dir>/ba<NN>/profil<k>.dat`. */
export const STEHLE_URL = "https://cdsarc.cds.unistra.fr/ftp/VI/98A/ba/";

/**
 * The profile files R08.T4.e reads, by path under {@link STEHLE_URL}, with the SHA-256 recorded at
 * their fetch on 2026-10-19 (none was given before).
 */
export const STEHLE_PROFILES: Readonly<Record<string, string>> = {
  "ba03/profil1.dat": "3c1bf05919b2f4d74e4b8c8df6024b8302d675a5ad4c2516d6b65e53f5f6bcce",
  "ba03/profil2.dat": "557fd011ef2e862d60938c640466a3c19a6e789468b96200ad453c6ad9c7b855",
  "ba03/profil3.dat": "e448c458276ab33f71c78b7238e12583e52b7aead2548c424df053aff4e4cf41",
  "ba03/profil4.dat": "2a47217af662260e68b0665a47ab7793fdce8501fc4b5e4d618304ad7def4a9f",
  "ba03/profil5.dat": "4548bfa2ce10a4238d6fd51912d68346e73275d0c1d0fa8e63b799d864599799",
  "ba03/profil6.dat": "7a99069fafde77f2383b3a433ee1c81ee2f3b8ec2971c3df9a29d760eb06c615",
  "ba03/profil7.dat": "e33e343854ba80f1d613c4900d5a28c071a443288231d84a972d12516592f794",
  "ba03/profil8.dat": "5698d1a6c85086f8b11eaf014a997643a12229b74dec72b45ee1cd0102c0d399",
  "ba03/profil9.dat": "2779e52b20527937eb62ae8e4741ee8fedccb93bc16833f23dd491abc7ef1780",
  "ba03/profil10.dat": "fc4d4ef7c67d5d60a5c2f68b5e1f1e668804846dc5b91134e596cff3e6fcb38d",
  "ba03/profil11.dat": "51a4aaeee99928144e64c9128c2a4593804429a7379463d8a4362bce2bcc7618",
  "ba04/profil1.dat": "41c550a2cda0ca99b20a9da3114752687353a6b1e8e98ca2a871fa85cf460636",
  "ba04/profil2.dat": "614e1f4246817b193ad607b775af35e0a247834fb95053681d608e43580ba2ba",
  "ba04/profil3.dat": "59507851ad2f49e9def52da43696425b549d14b9f110bc20a297c9e206989d6f",
  "ba04/profil4.dat": "2001174cbd8fa8a8fe39b2d31793129432e94501ee63976a854a58a35b8f09d3",
  "ba04/profil5.dat": "5f9167cbee332d5c1c876a57cc77c52582102e6f70414f4fdce564269580fa82",
  "ba04/profil6.dat": "47302eec65ad725103a0678a257929ef5cf312218db47d4b10fe0f1e96897df7",
  "ba04/profil7.dat": "96988663167a929994e7f12aa8f29b217331a609d96bc6ee797770983bf90b2b",
  "ba04/profil8.dat": "3580a2749a1114e391ed81622a81d4a4f360ec2176481a3ed709a0de6856bb95",
  "ba04/profil9.dat": "7d3c16a202bcf5bf69dd6492517fc1b67d9789978b5622b9a9088c5e7f635b72",
  "ba04/profil10.dat": "ec2be865bb5ea0c8d31bb909d39699a62e3bf231689ef8419f4994a7b40c80a3",
  "ba04/profil11.dat": "b9d9ca537bdbc555185bbfae94b0898e9412d60996d990ec7f20c74bb56335e7",
  "ba05/profil1.dat": "8ffcf2883daee27122663010a720e1fca152a112503f7ddf390040ddd7d6927d",
  "ba05/profil2.dat": "18c161fc2cde366c107c2b8d4a684dec15577aad717ca71067e0d82aaa76ae1a",
  "ba05/profil3.dat": "ba5cbf366fc2990d956696af43007c58953b9be504cfc7920bc07006aff08d12",
  "ba05/profil4.dat": "a3dbecbe34eb7e6151e9e263f8cc9bb97238d265b16b4773a2848d88d4a2712c",
  "ba05/profil5.dat": "c768c1ca1472e44030ab04c52f5815972f64ac74fde391972942d87fa2165ad3",
  "ba05/profil6.dat": "c138d880077251b7d2c21c59bd11c3152b1f3214ac98a74072a3c840fbc66330",
  "ba05/profil7.dat": "72aacf36605e79c75c845f86e62de9d0fad9e61ef119b6494b27b279c2406b37",
  "ba05/profil8.dat": "b95fff42e0197d6a903684928d2ef4db52cc4ba348d05c2b5c36e9c82a6e3c5b",
  "ba05/profil9.dat": "fa31cf59360b1b37e453697b79487ba5cdd480e0f7f37f57f2b2061872fc65ee",
  "ba05/profil10.dat": "9b9da294198e8719585e8586668de4af88be10b00f5d147cc24b3862501dbcdb",
  "ba05/profil11.dat": "8a55a5598782fecd70a8b0f13addb74b7d185fca7bd839411b58be4f1fea30ee",
  "ba06/profil1.dat": "c09518525ef05b29e9bef9d39e43bcbe42e23379e159a9f9ef9dff1154de61df",
  "ba06/profil2.dat": "3bda6fc844f9f9d416381e0650c7ffa5dd7bdb599b717ecba91adddcb3bcc92a",
  "ba06/profil3.dat": "6d38c03fa9fa664590a376c2591c21e0fa6aa072c8486aaf15b846c1f3285c1e",
  "ba06/profil4.dat": "6c709f49f804bf701c10479b31e648ec9db2c919f4b473b77d56709fe4f4d8e3",
  "ba06/profil5.dat": "8618032ff21930b611c2707c10699c2ac7715547d1213c4c91407dd1df7aa81d",
  "ba06/profil6.dat": "cbeac17bb54f99fbfcc0a9e0042971228f81438f11d791554e174c3b6092fed8",
  "ba06/profil7.dat": "62a6e50dd45558fb5ed1a5aef14761c7543dc43f5a59d39b0f6a563544985da8",
  "ba06/profil8.dat": "5bb1fbe6b7bb6bbb9a3af218ac9a35de49bb2f0c535f58bd2852718aab176736",
  "ba06/profil9.dat": "ef1492d8ad0ef656c0fb5a9e741cbc4eb8d4c6eb230b9e825f19248b4dc697e1",
  "ba06/profil10.dat": "b14d871a6e57f97bb277e09190dc5aaad8542113ae72127fe47f5c28ca9ad572",
  "ba06/profil11.dat": "762109665179aea87cc92fca499927c0c74dd86c00046d3f52bbe81cd2a2fb2f",
  "ba07/profil1.dat": "c19311aa993de5750ee030bc34b4f89c0c9861c52d820711432122d45dfc708a",
  "ba07/profil2.dat": "af24a26d7aab46e586582a7475401c3f46001d24aff5dcf208536964c2e4a8d6",
  "ba07/profil3.dat": "e77a3cd4a16c90bcd4a1d229911bb2ad9406f9c282801a37067434200a69d7b5",
  "ba07/profil4.dat": "5eff83ba53f6d9cde177c302a5cbf52707032cd490120a8ba93a278d5d1b4a29",
  "ba07/profil5.dat": "356c11ab0d642a30a98bcdb28aae430b6c8a412b340764f76ca9abe18eaa972e",
  "ba07/profil6.dat": "95e88d5a210afa1b037f52ef92b41276e68edfbf96aa39acc0b5c5a8a7fbd7ef",
  "ba07/profil7.dat": "0acbd3e10705a0e6f0761821ce309ebf3b95272317e1326d6633ebc6f68256be",
  "ba07/profil8.dat": "e0686dd510cc24275ef65029a15cbae2391de1f14805fc8479008612e59003b2",
  "ba07/profil9.dat": "e7a0319d353e2157abd82104ef31d9636879bc88d4e78b216dc6ea39520cf7f8",
  "ba07/profil10.dat": "04286a2760483e45968c413be7eaeef5b2cfeb4bf6e0d27613498a6a4d7a5afc",
  "ba07/profil11.dat": "4788d569482b0e9c0930b936de5821edf7bca2eb1fd2aa623057518149dab709",
  "ba08/profil1.dat": "128df7e1b92b596995a258044d8f892118fcaa5fba6cb78e765ae6acccb04663",
  "ba08/profil2.dat": "7f5db1964e8c5e367778568d50c2b72547712fb159efcbdcf26b8314c3222b90",
  "ba08/profil3.dat": "202520a8e19f2e05ad0e5d40e03b327e3eb1b12d8d82cfd704ab04351555d700",
  "ba08/profil4.dat": "3c3f09e492fd6f84ec128f1b44f28e0d58b10ca0f1343fb3b752a5758b508e4e",
  "ba08/profil5.dat": "aecd944a6a8922a0e3b5b8091974a7c93908ce86c8b3cdad344bce813dc53a34",
  "ba08/profil6.dat": "3cdf8d060779e5988563dc208e69ea1bcefde136c1bbbf4d5c8e84764d841ec7",
  "ba08/profil7.dat": "6393fde8fff5f7b6af2f0582227e4f7eff158fe20238ad4f9fab717bdd8fda25",
  "ba08/profil8.dat": "38b513c6f5eff05298f512e28d7457f2d0a79f068d219c2111f36952cc0a373d",
  "ba08/profil9.dat": "ec7692ca9d69223f877908c0245dae7cc6a00cf5a16192c1c86648f680e22edd",
  "ba08/profil10.dat": "1658d1e28af6542f002cb4a4d2c92b334f40ae032c31a0aad055a1fb22b96c03",
  "ba08/profil11.dat": "a72474a981249df255b18524aed8353509f745aa6147741bb57bb135d70a9748",
  "ba09/profil1.dat": "1a81f37ffc4bd0041f5348224fa492e058c8d9d87d06e405f5e52720d5b00467",
  "ba09/profil2.dat": "411de243c522e8efd6e12611495989bba5dc3130009e8c016a4d98474fd1230f",
  "ba09/profil3.dat": "871e12a1e44a4c1559d5e50d875b1702637f2c6afd7b0b769ec9d0d5642a9122",
  "ba09/profil4.dat": "3b7437d37da8a1e4f40b46fa7f363631a92792dfe9fc5d0231fe97b627c645a1",
  "ba09/profil5.dat": "596cd79032a448dac359ba9dbd445b46aef3337d7c9d0fcc658b8cad737a6bbe",
  "ba09/profil6.dat": "f726fcc095316d5932a15812211df4f5cd824f6ac0cafd931536b2e1c42107c3",
  "ba09/profil7.dat": "f549fc7afbd330d364f230d145ffd7ed0f536a84ea2acae9d55bb1e61d3edc46",
  "ba09/profil8.dat": "7738e9bea3e9eeaa5a12a3ea5886a984e8d26b791f60906c79bb936c9e728253",
  "ba09/profil9.dat": "e9b1164b22c00058da8960db5d4dd0ebc1bf49962ea96f6f3b9bec2b91e31700",
  "ba09/profil10.dat": "8600dbfc160d78453fe65b85b6560537f8deacb8e9504da042ac19552b53a5df",
  "ba09/profil11.dat": "9937feb697ccc4c78a11e33f88c1600b8e7c480d070710601c74663784dd1534",
  "ba10/profil1.dat": "6bf2ccf4102677e17e451cf9b4697f715dda20fc1568f82978fe6171f622c25d",
  "ba10/profil2.dat": "8cee86f1086ae1057771d6ca2e2219c92ed62907c452a3d03c183046fd146018",
  "ba10/profil3.dat": "079cb114281e5b7b5846d35d6bacc15a7ad09ea981b82cc206f71673cb66b243",
  "ba10/profil4.dat": "d0665c3292edc1965572a7178ff44f0d34d4d91a12e5322d3d3096bc63d733c6",
  "ba10/profil5.dat": "3e28d56d8e7e36f58cd43761aff645976de5fc3db1d860c28b2c195a4b87d77d",
  "ba10/profil6.dat": "915332f5e87180a6f601f1488e81f48ff9bff3c06fec411deff4ab1bf03dc721",
  "ba10/profil7.dat": "ffa90733c77bf7685d2f7c0dd033f371270308a53e4dd1221bb489fba622a01b",
  "ba10/profil8.dat": "35fe4b0fe1a1ed79de66e5350e49624a0ed9de0d80703780ff4d8797b7cc5dcf",
  "ba10/profil9.dat": "7c39becb644f9fde2caff00ffe5306bcd8671c1d85d844c12294498741b74066",
  "ba10/profil10.dat": "f1b37b1f7d1542d6f3254a32ad6db8a91d508d54df4a54e1679752c29b858205",
  "ba10/profil11.dat": "d9623a2bca01d3ec46e4e5e5534fdd8a763a47570636e20b889de781758e1cb9",
};

/** One profile file's header: its line, density and Stark half widths by temperature. */
export interface StarkProfileHeader {
  readonly lowerN: number;
  readonly upperN: number;
  /** The line's wavelength as the file gives it, Å (vacuum: H-alpha's 6,564.70). */
  readonly wavelengthA: number;
  readonly electronsPerCm3: number;
  /**
   * Δλ ÷ Δα, Å: the normal Holtsmark field F₀ = 1.25 × 10⁻⁹ n_e^(2/3) esu (n_e in cm⁻³; the
   * ReadMe's "Note on E0"), the reduced detuning being α = Δλ ÷ F₀.
   */
  readonly f0A: number;
  /** The temperatures of the file's first block, K. */
  readonly temperaturesK: ReadonlyArray<number>;
  /** The Stark half width at half maximum at each, in units of α (their "width Stark"). */
  readonly halfWidthsAlpha: ReadonlyArray<number>;
}

/** The numbers after a header line's label, at least one, each finite. */
function headerNumbers(line: string | undefined, label: RegExp, what: string): number[] {
  const rest = line?.replace(label, "");
  const values = rest?.trim().split(/\s+/u).map(Number) ?? [];
  if (
    rest === undefined ||
    rest === line ||
    values.length === 0 ||
    !values.every(Number.isFinite)
  ) {
    throw new Error(`a Stark profile's ${what} line is not as the catalogue's ReadMe gives it`);
  }
  return values;
}

/**
 * A profile file's header (the catalogue's ReadMe: line 1 the levels and wavelength, line 2 the
 * electron density, line 3 f0, line 6 the first block's temperatures and line 8 their Stark half
 * widths without Doppler).
 *
 * @throws Error for a header that is not as the ReadMe gives it.
 */
export function parseStarkHeader(text: string): StarkProfileHeader {
  const lines = text.split(/\r?\n/u);
  const levels =
    /N LOWER\s*=\s*(\d+)\s+N UPPER\s*=\s*(\d+)\s+WAVELENGTH \(angstroms\)\s*=\s*(\S+)/u.exec(
      lines[0] ?? "",
    );
  const [lowerN, upperN, wavelengthA] = [levels?.[1], levels?.[2], levels?.[3]].map(Number);
  const [electronsPerCm3] = headerNumbers(
    lines[1],
    /^\s*electronic density\(1\/cm\*\*3\)/u,
    "density",
  );
  const [f0A] = headerNumbers(lines[2], /^.*=f0/u, "f0");
  const temperaturesK = headerNumbers(lines[5], /^T\(K\)/u, "temperature");
  const halfWidthsAlpha = headerNumbers(lines[7], /^width Stark/u, "width");
  if (
    levels === null ||
    lowerN === undefined ||
    upperN === undefined ||
    wavelengthA === undefined ||
    electronsPerCm3 === undefined ||
    f0A === undefined ||
    !Number.isFinite(wavelengthA) ||
    temperaturesK.length !== halfWidthsAlpha.length
  ) {
    throw new Error("a Stark profile's header is not as the catalogue's ReadMe gives it");
  }
  return { lowerN, upperN, wavelengthA, electronsPerCm3, f0A, temperaturesK, halfWidthsAlpha };
}

/** A Balmer line's Stark half widths over the densities and temperatures read. */
export interface BalmerStarkLine {
  readonly upperN: number;
  readonly wavelengthA: number;
  /** The densities, cm⁻³, ascending. */
  readonly electronsPerCm3: ReadonlyArray<number>;
  /** The temperatures, K, ascending. */
  readonly temperaturesK: ReadonlyArray<number>;
  /** `halfWidthsA[d][t]`: the half width at half maximum, Å. */
  readonly halfWidthsA: ReadonlyArray<ReadonlyArray<number>>;
}

/**
 * A line's headers, one a density, as one line's table.
 *
 * @throws Error for headers of more than one line or temperature grid, or of under two densities,
 * which no interpolation in ln n_e can take.
 */
export function balmerStarkLine(headers: ReadonlyArray<StarkProfileHeader>): BalmerStarkLine {
  const sorted = headers.toSorted((a, b) => a.electronsPerCm3 - b.electronsPerCm3);
  const first = sorted[0];
  if (sorted.length < 2) {
    throw new Error("a Balmer line's Stark table takes at least two densities");
  }
  if (
    first === undefined ||
    sorted.some(
      (h) =>
        h.upperN !== first.upperN ||
        h.lowerN !== 2 ||
        h.temperaturesK.join() !== first.temperaturesK.join(),
    )
  ) {
    throw new Error(
      "a Balmer line's Stark headers are not one Balmer line on one temperature grid",
    );
  }
  return {
    upperN: first.upperN,
    wavelengthA: first.wavelengthA,
    electronsPerCm3: sorted.map((h) => h.electronsPerCm3),
    temperaturesK: first.temperaturesK,
    halfWidthsA: sorted.map((h) => h.halfWidthsAlpha.map((w) => w * h.f0A)),
  };
}

/** The Balmer lines' Stark tables from the checked files ({@link STEHLE_PROFILES}), H3 first. */
export function readBalmerStark(dir: string): ReadonlyArray<BalmerStarkLine> {
  const byLine = new Map<string, StarkProfileHeader[]>();
  for (const [path, sha256] of Object.entries(STEHLE_PROFILES)) {
    const line = path.split("/")[0] ?? "";
    const headers = byLine.get(line) ?? [];
    headers.push(parseStarkHeader(readCheckedText(join(dir, path), sha256)));
    byLine.set(line, headers);
  }
  return [...byLine.values()].map(balmerStarkLine).toSorted((a, b) => a.upperN - b.upperN);
}

/** The index of the interval of an ascending grid that holds x, held to its end intervals. */
function intervalOf(grid: ReadonlyArray<number>, x: number): number {
  let i = 0;
  while (i < grid.length - 2 && (grid[i + 1] ?? Infinity) < x) {
    i += 1;
  }
  return i;
}

/**
 * A Balmer line's Stark half width at half maximum at an electron density and a temperature, Å:
 * ln w linear in ln n_e between the tabulated densities and extrapolated so from the end intervals
 * (below 10¹⁰ cm⁻³ the slope of the first half decade, near n_e^0.9 for Hα), and linear in T
 * between the tabulated temperatures, held beyond them.
 */
export function starkHalfWidthA(
  line: BalmerStarkLine,
  electronsPerCm3: number,
  temperatureK: number,
): number {
  const temps = line.temperaturesK;
  const t = intervalOf(temps, temperatureK);
  const tNext = Math.min(t + 1, temps.length - 1);
  const t0 = temps[t];
  const t1 = temps[tNext];
  if (t0 === undefined || t1 === undefined) {
    throw new Error(`H${line.upperN}'s Stark table holds no temperature`);
  }
  const along = t1 > t0 ? Math.min(Math.max((temperatureK - t0) / (t1 - t0), 0), 1) : 0;
  const atDensity = (d: number): number => {
    const a = line.halfWidthsA[d]?.[t];
    const b = line.halfWidthsA[d]?.[tNext];
    if (a === undefined || b === undefined) {
      throw new Error(`H${line.upperN}'s Stark table holds no half width at density ${d}`);
    }
    return a + (b - a) * along;
  };
  const densities = line.electronsPerCm3;
  const d = intervalOf(densities, electronsPerCm3);
  const [n0, n1] = [densities[d], densities[d + 1]];
  if (n0 === undefined || n1 === undefined) {
    throw new Error(`H${line.upperN}'s Stark table holds under two densities`);
  }
  const x0 = Math.log(n0);
  const x1 = Math.log(n1);
  const y0 = Math.log(atDensity(d));
  const y1 = Math.log(atDensity(d + 1));
  return Math.exp(y0 + ((y1 - y0) * (Math.log(electronsPerCm3) - x0)) / (x1 - x0));
}

/** A half width in Å at a wavelength in Å as a half width in wavenumber, cm⁻¹: Δλ ÷ λ² × 10⁸. */
export function angstromHalfWidthPerCm(halfWidthA: number, wavelengthA: number): number {
  return (halfWidthA / wavelengthA ** 2) * 1e8;
}
