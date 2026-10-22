/**
 * The reader of `TERRAIN TEST`'s field (plan R10, R10.T16.d; decision-r10-t16-contracts §4–5):
 * `hyperion_surface`'s `tests/golden/worker/terrain_test.golden`, the Moon-like world's payloads
 * with the answers R09.T19.b gives with them, which the kept scene's fixture link serves in the
 * server's place and from which the scene takes `TEST MOON`'s ID, detail seed and figure.
 *
 * @remarks
 * The file's format is its writer's (`crates/hyperion-surface/src/worker/golden.rs`, "The terrain
 * test's format"): `body` and `seed`; the field header as `SurfaceHeaderDto` carries it, a `dto`
 * line a member, every float as `0x` and its 64 bits in hexadecimal (a scalar's line ending in a
 * `#` comment, which is not read), each palette entry's `FeO_T` after its liquid record, and a
 * member or an `FeO_T` not modelled as `-`, left out as the wire leaves it out; the coverage at each revision (`survey`); each payload's
 * revision and `have_revision` (`payload`), its answer's coverage (`cover`) and its blocks
 * (`block`, hexadecimal); and what each block of the first payload and the delta changes in a
 * worker's field (`change`, `held`). Where the header carries a maturity, as `TEST MOON`
 * weathered's (`terrain_test_weathered.golden`, R10.T1.c's client half) does, five lines follow:
 * the primary crust's drawn albedo (`crust_albedo`), three patches' maturity layers and nodes as
 * digests (`maturity_patch`) and the class map's (`class_map_maturity`). The server crate's test
 * holds the header lines and covers to the server's own conversion, so the answers built here are
 * the server's. The reader is pure, and throws on any line it cannot read, naming it: the file is
 * committed, so a failure is a bug.
 */
import type {
  BodyIdHex,
  ClimateModelDto,
  CoverDto,
  CoverRangeDto,
  DetailSeedHex,
  PaletteEntryDto,
  PaletteRoleDto,
  PrecipitationSourceDto,
  SurfaceHeaderDto,
} from "@hyperion/protocol";

import type { Face, PatchKey } from "../terrain/patchKey";

/** One payload of the field: R09.T19.b's answer to one `surface_field`. */
export interface TerrainGoldenPayload {
  /** The body's coverage revision the payload reflects, from 1. */
  readonly revision: number;
  /** The `have_revision` it answers: `null` for the whole field. */
  readonly have: number | null;
  /** The cells its blocks hold, each with its resolution code (the answer's `coverage`). */
  readonly coverage: CoverDto;
  /** Its blocks, in order, each as lowercase hexadecimal ({@link blockBytes} decodes one). */
  readonly blocksHex: ReadonlyArray<string>;
}

/** What one block changed in a worker's field: the runs of cells it surveyed anew and brought. */
export interface TerrainGoldenChange {
  /** The cells it surveyed anew, as half-open runs of cell indices (`SurveyChange::runs`). */
  readonly surveyed: ReadonlyArray<readonly [number, number]>;
  /** The cells it brought, surveyed or margin (`SurveyChange::held_runs`). */
  readonly held: ReadonlyArray<readonly [number, number]>;
}

/**
 * A patch whose maturity layer the golden pins (R10.T1.c's client half): baked with the weights at
 * the fixture's seed over payloads 1 and 2's blocks.
 */
export interface TerrainGoldenMaturityPatch {
  /** The patch holding the second survey's point at band level 8, 13 or the band limit's. */
  readonly key: PatchKey;
  /**
   * The FNV-1a digest of the bake's maturity codes (`BakedTerrain.maturity()`), each as an `f64`:
   * `f64Digest(Float64Array.from(codes))`.
   */
  readonly codesDigest: bigint;
  /**
   * The FNV-1a digest of `SurfaceFields.maturityNodes` for the bake's own palette and level, over
   * its `Float32Array`'s bits (`f32Digest`).
   */
  readonly nodesDigest: bigint;
}

/** What the golden pins of a field whose header carries a maturity (R10.T1.c's client half). */
export interface TerrainGoldenMaturity {
  /**
   * The primary crust's regolith's drawn normal albedo at the body's background maturity at band
   * level 0, in the Johnson B, V and R bands: the ground the class map leaves to `elsewhere`.
   */
  readonly crustAlbedoBvr: readonly [number, number, number];
  /** The patches, at band levels 8 and 13 and at the band limit's, in that order. */
  readonly patches: ReadonlyArray<TerrainGoldenMaturityPatch>;
  /**
   * The class map's (at the fixture's seed) maturity: its texels' codes' digest, as a patch's, and
   * its classes' band-0 nodes' (`ClassMapAnswer.nodes()`, `f32Digest`).
   */
  readonly classMap: { readonly codesDigest: bigint; readonly nodesDigest: bigint };
}

/** `TERRAIN TEST`'s field as the golden holds it. */
export interface TerrainGolden {
  /** The generator version the file was blessed at (its header). */
  readonly generatorVersion: number;
  /** The field's body: `TEST MOON`. */
  readonly body: BodyIdHex;
  /** The body's detail seed. */
  readonly seed: DetailSeedHex;
  /** The field's header, as R09.T19.a's `surface_header` gives it. */
  readonly header: SurfaceHeaderDto;
  /** The body's coverage at each revision, from 1 at index 0, as the server's survey log folds it. */
  readonly surveys: ReadonlyArray<CoverDto>;
  /**
   * The payloads: the whole field at revision 1, the delta from 1 to 2, the whole field at 2, and
   * the answer to an ask at the revision held (block 0 alone, the same at either revision).
   */
  readonly payloads: ReadonlyArray<TerrainGoldenPayload>;
  /** What each block of the first payload, then of the delta, changed, in the order posted. */
  readonly changes: ReadonlyArray<TerrainGoldenChange>;
  /** The maturity's lines; absent for a field whose header carries none, as `TERRAIN TEST`'s. */
  readonly maturity?: TerrainGoldenMaturity;
}

const HEADER_PREFIX = "# generator_version = ";

const CLIMATE_MODELS: ReadonlyArray<ClimateModelDto> = [
  "energy_balance",
  "locked_energy_balance",
  "radiative_equilibrium",
  "isothermal",
];

const PRECIPITATION_SOURCES: ReadonlyArray<PrecipitationSourceDto> = ["heuristic"];

const PALETTE_ROLES: ReadonlyArray<PaletteRoleDto> = [
  "primary_crust",
  "secondary_crust",
  "tertiary_crust",
  "province",
  "ice",
  "liquid",
  "cover",
  "deposit",
  "bulk",
];

/** A line the reader could not read. */
function refused(line: number, why: string): Error {
  return new Error(`terrain_test.golden, line ${String(line)}: ${why}`);
}

/** A float from its golden form, `0x` and 16 lowercase hexadecimal digits of its bits. */
function floatOf(token: string | undefined, line: number): number {
  if (token === undefined || !/^0x[0-9a-f]{16}$/u.test(token)) {
    throw refused(line, `${String(token)} is not a float's bits`);
  }
  const view = new DataView(new ArrayBuffer(8));
  view.setBigUint64(0, BigInt(token));
  return view.getFloat64(0);
}

/** A 64-bit digest from its golden form, `0x` and 16 lowercase hexadecimal digits. */
function digestOf(token: string | undefined, line: number): bigint {
  if (token === undefined || !/^0x[0-9a-f]{16}$/u.test(token)) {
    throw refused(line, `${String(token)} is not a digest's 64 bits`);
  }
  return BigInt(token);
}

const FACES: ReadonlyArray<Face> = [0, 1, 2, 3, 4, 5];

/** A cube face's index, 0 to 5. */
function faceOf(token: string | undefined, line: number): Face {
  const index = wholeOf(token, line);
  const face = FACES.find((f) => f === index);
  if (face === undefined) {
    throw refused(line, `${String(token)} is not a face`);
  }
  return face;
}

/** A whole number of at least 0, in decimal. */
function wholeOf(token: string | undefined, line: number): number {
  if (token === undefined || !/^(0|[1-9][0-9]*)$/u.test(token)) {
    throw refused(line, `${String(token)} is not a whole number`);
  }
  return Number(token);
}

/** One of `names`, as the wire spells it. */
function nameOf<T extends string>(
  token: string | undefined,
  names: ReadonlyArray<T>,
  line: number,
): T {
  const found = names.find((name) => name === token);
  if (found === undefined) {
    throw refused(line, `${String(token)} is not one of ${names.join(", ")}`);
  }
  return found;
}

/** A cover from its golden runs, each `<first>-<past last>:<code>`, `-` for none. */
function coverOf(runs: ReadonlyArray<string>, line: number): CoverDto {
  if (runs.length === 1 && runs[0] === "-") {
    return { ranges: [] };
  }
  const ranges = runs.map((run): CoverRangeDto => {
    const match = /^(\d+)-(\d+):(\d+)$/u.exec(run);
    if (match === null) {
      throw refused(line, `${run} is not a run with its code`);
    }
    return {
      start: wholeOf(match[1], line),
      end: wholeOf(match[2], line),
      resolution: wholeOf(match[3], line),
    };
  });
  return { ranges };
}

/** Runs of cells from their golden form, each `<first>-<past last>`, `-` for none. */
function runsOf(runs: ReadonlyArray<string>, line: number): Array<readonly [number, number]> {
  if (runs.length === 1 && runs[0] === "-") {
    return [];
  }
  return runs.map((run): readonly [number, number] => {
    const match = /^(\d+)-(\d+)$/u.exec(run);
    if (match === null) {
      throw refused(line, `${run} is not a run`);
    }
    return [wholeOf(match[1], line), wholeOf(match[2], line)];
  });
}

/** A header member's value, with the line that gave it. */
interface MemberToken {
  readonly value: string;
  readonly line: number;
}

/** The header's members, as its `dto` lines give them, before they are checked whole. */
interface HeaderParts {
  readonly scalars: Map<string, MemberToken>;
  readonly palette: PaletteEntryDto[];
}

/** Takes one `dto` line's tokens (after `dto`). */
function takeDto(tokens: ReadonlyArray<string>, parts: HeaderParts, line: number): void {
  const [key, ...rest] = tokens;
  if (key === undefined) {
    throw refused(line, "a dto line names no member");
  }
  if (key !== "palette") {
    const [value, ...more] = rest;
    if (value === undefined || more.length > 0) {
      throw refused(line, `the member ${key} takes one value`);
    }
    if (parts.scalars.has(key)) {
      throw refused(line, `the member ${key} is given twice`);
    }
    parts.scalars.set(key, { value, line });
    return;
  }
  const [index, substance, role, b, v, r, row, liquid, feo, ...more] = rest;
  if (substance === undefined || feo === undefined || more.length > 0) {
    throw refused(
      line,
      "a palette entry is <index> <substance> <role> <B> <V> <R> <row> <liquid> <FeO_T>",
    );
  }
  if (wholeOf(index, line) !== parts.palette.length) {
    throw refused(line, `palette entry ${String(index)} is out of its order`);
  }
  if (liquid !== "-") {
    throw refused(line, "the terrain test's format holds no liquid record");
  }
  parts.palette.push({
    substance,
    role: nameOf(role, PALETTE_ROLES, line),
    normal_albedo_bvr: [floatOf(b, line), floatOf(v, line), floatOf(r, line)],
    phase_row: wholeOf(row, line),
    liquid: null,
    // An `FeO_T` not modelled is left out, as the wire leaves it out.
    ...(feo === "-" ? {} : { feo_mass_fraction: floatOf(feo, line) }),
  });
}

/** The header from its members, each required one present and none left over. */
function headerOf(parts: HeaderParts): SurfaceHeaderDto {
  const { scalars } = parts;
  const used = new Set<string>();
  const member = (key: string): MemberToken => {
    const token = scalars.get(key);
    if (token === undefined) {
      throw new Error(`terrain_test.golden: the header has no ${key}`);
    }
    used.add(key);
    return token;
  };
  const text = (key: string): string => member(key).value;
  const whole = (key: string): number => {
    const { value, line } = member(key);
    return wholeOf(value, line);
  };
  const float = (key: string): number => {
    const { value, line } = member(key);
    return floatOf(value, line);
  };
  const absent = (key: string): boolean => member(key).value === "-";
  const header: SurfaceHeaderDto = {
    body: text("body"),
    level: whole("level"),
    radius_m: float("radius_m"),
    equatorial_radius_m: float("equatorial_radius_m"),
    polar_radius_m: float("polar_radius_m"),
    boundary_diameter_m: float("boundary_diameter_m"),
    sea_level_m: float("sea_level_m"),
    lapse_rate_k_per_m: float("lapse_rate_k_per_m"),
    realised_sigma_h_m: float("realised_sigma_h_m"),
    realised_relief_m: float("realised_relief_m"),
    months: whole("months"),
    season_eccentricity: float("season_eccentricity"),
    reference_temperature_k: float("reference_temperature_k"),
    surface_age_gyr: float("surface_age_gyr"),
    surface_pressure_pa: float("surface_pressure_pa"),
    albedo_scale: absent("albedo_scale") ? null : float("albedo_scale"),
    climate_model: nameOf(text("climate_model"), CLIMATE_MODELS, member("climate_model").line),
    precipitation: nameOf(
      text("precipitation"),
      PRECIPITATION_SOURCES,
      member("precipitation").line,
    ),
    palette: parts.palette,
    main_liquid: absent("main_liquid") ? null : whole("main_liquid"),
    // A member not modelled is left out, as the wire leaves it out.
    ...(absent("maturity") ? {} : { maturity: float("maturity") }),
    ...(absent("organic_cover") ? {} : { organic_cover: float("organic_cover") }),
    ...(absent("weathering_rate") ? {} : { weathering_rate: float("weathering_rate") }),
  };
  const unread = [...scalars.keys()].filter((key) => !used.has(key));
  if (unread.length > 0) {
    throw new Error(`terrain_test.golden: the header has unknown members ${unread.join(", ")}`);
  }
  return header;
}

/**
 * The terrain test's field from the golden's text.
 *
 * @throws Error naming the first line it cannot read, or a member, payload or block missing.
 */
export function parseTerrainGolden(text: string): TerrainGolden {
  let generatorVersion: number | null = null;
  let body: string | null = null;
  let seed: string | null = null;
  const parts: HeaderParts = { scalars: new Map(), palette: [] };
  const surveys: CoverDto[] = [];
  const payloads: Array<{
    revision: number;
    have: number | null;
    coverage: CoverDto | null;
    blocksHex: string[];
  }> = [];
  const changes: Array<{
    surveyed: Array<readonly [number, number]> | null;
    held: Array<readonly [number, number]> | null;
  }> = [];
  let maturity: {
    crustAlbedoBvr: readonly [number, number, number];
    patches: TerrainGoldenMaturityPatch[];
    classMap: TerrainGoldenMaturity["classMap"] | null;
  } | null = null;
  // A checkout with CRLF line ends (Git for Windows' default) reads as one with LF
  // (decision-r10-t16d-part1, cross-platform).
  const lines = text.split(/\r?\n/u);
  for (const [at, raw] of lines.entries()) {
    const line = at + 1;
    if (raw.startsWith(HEADER_PREFIX)) {
      generatorVersion = wholeOf(raw.slice(HEADER_PREFIX.length), line);
      continue;
    }
    const comment = raw.indexOf("  #");
    const tokens = (comment === -1 ? raw : raw.slice(0, comment))
      .split(" ")
      .filter((t) => t !== "");
    const [kind, first, second, third, ...rest] = tokens;
    switch (kind) {
      case undefined:
        break;
      case "body":
        body = first ?? null;
        break;
      case "seed":
        seed = first ?? null;
        break;
      case "dto":
        takeDto(tokens.slice(1), parts, line);
        break;
      case "survey": {
        if (wholeOf(first, line) !== surveys.length + 1) {
          throw refused(line, `survey ${String(first)} is out of its order`);
        }
        surveys.push(coverOf(tokens.slice(2), line));
        break;
      }
      case "payload": {
        if (
          wholeOf(first, line) !== payloads.length + 1 ||
          third === undefined ||
          rest.length > 0
        ) {
          throw refused(line, "a payload is <n> <revision> <have>, in order");
        }
        payloads.push({
          revision: wholeOf(second, line),
          have: third === "-" ? null : wholeOf(third, line),
          coverage: null,
          blocksHex: [],
        });
        break;
      }
      case "cover":
      case "block": {
        const payload = payloads.at(-1);
        if (payload === undefined || wholeOf(first, line) !== payloads.length) {
          throw refused(line, `a ${kind} line follows its payload's`);
        }
        if (kind === "cover") {
          payload.coverage = coverOf(tokens.slice(2), line);
        } else if (wholeOf(second, line) !== payload.blocksHex.length || third === undefined) {
          throw refused(line, "a block is <payload> <index> <bytes>, in order");
        } else if (!/^([0-9a-f]{2})+$/u.test(third)) {
          throw refused(line, "a block's bytes are lowercase hexadecimal");
        } else {
          payload.blocksHex.push(third);
        }
        break;
      }
      case "change":
      case "held": {
        const ordinal = wholeOf(first, line);
        if (kind === "change") {
          if (ordinal !== changes.length) {
            throw refused(line, `change ${String(ordinal)} is out of its order`);
          }
          changes.push({ surveyed: runsOf(tokens.slice(2), line), held: null });
        } else {
          const change = changes.at(-1);
          if (change === undefined || ordinal !== changes.length - 1) {
            throw refused(line, `held ${String(ordinal)} follows its change`);
          }
          change.held = runsOf(tokens.slice(2), line);
        }
        break;
      }
      case "crust_albedo": {
        if (maturity !== null || third === undefined || rest.length > 0) {
          throw refused(line, "one crust_albedo line, <B> <V> <R>, starts the maturity's lines");
        }
        maturity = {
          crustAlbedoBvr: [floatOf(first, line), floatOf(second, line), floatOf(third, line)],
          patches: [],
          classMap: null,
        };
        break;
      }
      case "maturity_patch": {
        const [face, level, i, j, codes, nodes, ...more] = tokens.slice(1);
        if (
          maturity === null ||
          maturity.classMap !== null ||
          nodes === undefined ||
          more.length > 0
        ) {
          throw refused(
            line,
            "a maturity_patch is <face> <level> <i> <j> <codes> <nodes>, between the crust_albedo and class_map_maturity lines",
          );
        }
        maturity.patches.push({
          key: {
            face: faceOf(face, line),
            level: wholeOf(level, line),
            i: wholeOf(i, line),
            j: wholeOf(j, line),
          },
          codesDigest: digestOf(codes, line),
          nodesDigest: digestOf(nodes, line),
        });
        break;
      }
      case "class_map_maturity": {
        if (maturity === null || maturity.classMap !== null || third !== undefined) {
          throw refused(line, "one class_map_maturity line, <codes> <nodes>, ends the maturity's");
        }
        maturity.classMap = {
          codesDigest: digestOf(first, line),
          nodesDigest: digestOf(second, line),
        };
        break;
      }
      default:
        throw refused(line, `${kind} is not a line of the terrain test's format`);
    }
  }
  if (generatorVersion === null || body === null || seed === null) {
    throw new Error("terrain_test.golden: its header, body or seed is missing");
  }
  const header = headerOf(parts);
  if ((header.maturity !== undefined) !== (maturity !== null)) {
    throw new Error(
      "terrain_test.golden: the maturity's lines follow a header with a maturity, and only one",
    );
  }
  const classMap = maturity?.classMap ?? null;
  if (maturity !== null && (classMap === null || maturity.patches.length === 0)) {
    throw new Error("terrain_test.golden: the maturity's lines have no patch or no class map's");
  }
  return {
    generatorVersion,
    body,
    seed,
    header,
    surveys,
    payloads: payloads.map((payload, n) => {
      if (payload.coverage === null || payload.blocksHex.length === 0) {
        throw new Error(`terrain_test.golden: payload ${String(n + 1)} has no cover or no block`);
      }
      return {
        revision: payload.revision,
        have: payload.have,
        coverage: payload.coverage,
        blocksHex: payload.blocksHex,
      };
    }),
    changes: changes.map((change, n) => {
      if (change.surveyed === null || change.held === null) {
        throw new Error(`terrain_test.golden: change ${String(n)} has no held runs`);
      }
      return { surveyed: change.surveyed, held: change.held };
    }),
    ...(maturity === null || classMap === null
      ? {}
      : {
          maturity: {
            crustAlbedoBvr: maturity.crustAlbedoBvr,
            patches: maturity.patches,
            classMap,
          },
        }),
  };
}

/** A block's bytes from its golden form, in a buffer of their own, which a caller may transfer. */
export function blockBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let n = 0; n < bytes.length; n += 1) {
    bytes[n] = Number.parseInt(hex.slice(2 * n, 2 * n + 2), 16);
  }
  return bytes;
}
