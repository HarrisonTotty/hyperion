/**
 * R07.T8.a's six air bodies at the 3 px switch (plan R07, R07.T8.a; plan R08, R08.T16.b): each one's
 * disc table as the optics worker bakes it, held in a committed fixture (`view/atmosphere/fixtures/
 * air-switch/`), and the body its disc and its point are drawn as.
 *
 * @remarks
 * - **The cases.** `ATMOSPHERE TEST`'s `TEST EARTH` (the measured Earth on WGS 84), `TEST MARS`,
 *   `TEST TITAN` and `TEST VENUS` (each a grey Lambert sphere of its medium's ground albedo, as the
 *   scene draws it), and two served worlds of the Dev Fixture universe (seed 0x4d2): the temperate
 *   `41ffecb200000002.0400` under a class map, and `42002cae00000004.0300`, whose medium carries its
 *   water deck, under its record's photometry, as a test server's `body_detail` at `full` sent them.
 * - **The tables** are baked as `test/discAirCases.ts`' `bakedDiscAir` bakes a case: J by the thick
 *   bake's solver under the reference Sun, every one of a single band and light slot. `TEST VENUS`
 *   is baked over its whole column, not split at its deck as the scene routes it: the table's
 *   transmittance is 0 there either way, and the deck's light is the bake's. The served worlds'
 *   media are assembled from their frames as the optics worker assembles a reply, their line
 *   absorbers on the committed k-distributions, their ground the record's p q.
 * - **The class map** is synthetic: an ocean and a land class over every texel, about half each,
 *   their albedos about those of a temperate world's drawn ground, the record's law `elsewhere`.
 * - **By hand.** `airSwitchCases.test.ts` bakes them under `HYPERION_AIR_SWITCH=1`, and with
 *   `HYPERION_BLESS=1` writes the fixtures (some 2 minutes, pod, 32 vCPU), each served frame read
 *   from its own committed fixture.
 */

import { decodeServerMessage, type SystemIdHex } from "@hyperion/protocol";

import servedClassMap from "../view/atmosphere/fixtures/air-switch/served-class-map.json" with { type: "json" };
import servedDecked from "../view/atmosphere/fixtures/air-switch/served-decked.json" with { type: "json" };
import testEarth from "../view/atmosphere/fixtures/air-switch/test-earth.json" with { type: "json" };
import testMars from "../view/atmosphere/fixtures/air-switch/test-mars.json" with { type: "json" };
import testTitan from "../view/atmosphere/fixtures/air-switch/test-titan.json" with { type: "json" };
import testVenus from "../view/atmosphere/fixtures/air-switch/test-venus.json" with { type: "json" };

import { type BodyPhotometry, modelledPhotometry } from "../view/appearance/fromWire";
import type { DiscSurface } from "../view/appearance/bodyAppearance";
import type { PhotometricLaw } from "../view/appearance/law";
import { lawFor } from "../view/appearance/phase";
import { loadKDistributions } from "../view/atmosphere/absorbers/kDistribution";
import {
  absorberCurveSlots,
  assembleMedium,
  lineSpeciesOf,
  withCurves,
} from "../view/atmosphere/assemble";
import { MARS_FIT_COLUMN } from "../view/atmosphere/channels";
import type { DiscAir } from "../view/atmosphere/discReflectance";
import { DISC_LAYER_VALUES, type DiscReflectanceTable } from "../view/atmosphere/discTable";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  EARTH_REFERENCE_SOURCES,
} from "../view/atmosphere/earth";
import type { SpheroidFigure } from "../view/atmosphere/hillaire";
import { REFERENCE_CASES, type ReferenceCase } from "../view/atmosphere/reference/cases";
import {
  thickReferenceCases,
  TITAN_GM_M3S2,
  TITAN_RADIUS_M,
  titanClassReferenceCase,
  VENUS_GRAVITY_MS2,
} from "../view/atmosphere/reference/thickCases";
import { sphereBakeBody } from "../view/atmosphere/thick/keptBake";
import { classMapSurface, type ClassMapTexels, packClassMap } from "../view/bodies/discSurface";
import { cos, sin } from "../lib/pinnedMath";
import { fromHalfBits, toHalfBits } from "../view/sky/half";
import { faceUvToXyz, stToUv } from "../view/terrain/cube";
import { earthBody } from "./atmosphereFixtures";
import { detailOf, recordInput } from "./atmosphereRecords";
import { bakedDiscAir, type DiscAirCase } from "./discAirCases";
import { inlinePart } from "./kDistributionFixtures";
import { UNREAD_TEXTURES } from "./normalMapFixtures";

/** The cases, by their fixtures' names. */
export const AIR_SWITCH_CASES = [
  "test-earth",
  "test-mars",
  "test-titan",
  "test-venus",
  "served-class-map",
  "served-decked",
] as const;

/** A case's name. */
export type AirSwitchCaseName = (typeof AIR_SWITCH_CASES)[number];

/** Each case's committed fixture, as its JSON imports, and the file it is, from `view/bodies/`. */
export const AIR_SWITCH_FIXTURES: Readonly<
  Record<AirSwitchCaseName, { readonly json: unknown; readonly file: string }>
> = {
  "test-earth": { json: testEarth, file: "../atmosphere/fixtures/air-switch/test-earth.json" },
  "test-mars": { json: testMars, file: "../atmosphere/fixtures/air-switch/test-mars.json" },
  "test-titan": { json: testTitan, file: "../atmosphere/fixtures/air-switch/test-titan.json" },
  "test-venus": { json: testVenus, file: "../atmosphere/fixtures/air-switch/test-venus.json" },
  "served-class-map": {
    json: servedClassMap,
    file: "../atmosphere/fixtures/air-switch/served-class-map.json",
  },
  "served-decked": {
    json: servedDecked,
    file: "../atmosphere/fixtures/air-switch/served-decked.json",
  },
};

/** A case's disc table as its fixture holds it: each value's half bits, little-endian, in base64. */
export interface AirSwitchTable {
  readonly bands: number;
  readonly slots: number;
  readonly sunFloor: number;
  readonly halves: string;
}

/** A case's fixture as its file holds it, which a bless writes byte for byte. */
export interface AirSwitchFile {
  readonly source: string;
  /** What the case is shown as in a test's name. */
  readonly name: string;
  /** The body's figure, the medium's own. */
  readonly figure: SpheroidFigure;
  /** A kept fixture's ground albedo, its Lambert photometry's A; `null` for a served world. */
  readonly groundAlbedo: number | null;
  /** A served world's `body_detail` frame as the server sent it; `null` for a kept fixture. */
  readonly frame: unknown;
  /** Whether it is drawn under the synthetic class map. */
  readonly classMap: boolean;
  readonly table: AirSwitchTable;
}

/**
 * What a case's body is drawn from: a kept fixture's grey Lambert ground, or a served world's
 * frame.
 */
export type AirSwitchSource =
  | { readonly kind: "kept"; readonly groundAlbedo: number }
  | { readonly kind: "served"; readonly frame: unknown };

/** A case's fixture as read: its file's fields, its body's source one of its two kinds. */
export interface AirSwitchFixture {
  readonly name: string;
  readonly figure: SpheroidFigure;
  readonly body: AirSwitchSource;
  readonly classMap: boolean;
  readonly table: AirSwitchTable;
}

/** A case as its disc and point are drawn. */
export interface AirSwitchBody {
  readonly name: string;
  readonly figure: SpheroidFigure;
  readonly photometry: BodyPhotometry;
  /** Its class map, where it is drawn under one. */
  readonly surface?: DiscSurface;
  readonly air: DiscAir;
}

/** Whether `value` is an object with string keys. */
function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** `value[key]` as a finite number, or a thrown error naming it. */
function numberAt(value: Readonly<Record<string, unknown>>, key: string): number {
  const found = value[key];
  if (typeof found !== "number" || !Number.isFinite(found)) {
    throw new Error(`an air switch fixture's ${key} is not a finite number`);
  }
  return found;
}

/** `value[key]` as a string, or a thrown error naming it. */
function stringAt(value: Readonly<Record<string, unknown>>, key: string): string {
  const found = value[key];
  if (typeof found !== "string") {
    throw new Error(`an air switch fixture's ${key} is not a string`);
  }
  return found;
}

/** `value[key]` as an object, or a thrown error naming it. */
function recordAt(
  value: Readonly<Record<string, unknown>>,
  key: string,
): Readonly<Record<string, unknown>> {
  const found = value[key];
  if (!isRecord(found)) {
    throw new Error(`an air switch fixture's ${key} is not an object`);
  }
  return found;
}

/**
 * A committed fixture's JSON as a fixture: a kept fixture with its ground albedo and no frame, or a
 * served world with its frame and no ground albedo.
 *
 * @throws Error naming the first field that is not of its kind, or a fixture that is neither.
 */
export function airSwitchFixtureOf(json: unknown): AirSwitchFixture {
  if (!isRecord(json)) {
    throw new Error("an air switch fixture is an object");
  }
  const figure = recordAt(json, "figure");
  const table = recordAt(json, "table");
  const ground = json["groundAlbedo"];
  const frame = json["frame"] ?? null;
  const classMap = json["classMap"];
  if (typeof classMap !== "boolean") {
    throw new Error("an air switch fixture's classMap is not a boolean");
  }
  let body: AirSwitchSource;
  if (ground === null && isRecord(frame)) {
    body = { kind: "served", frame };
  } else if (ground !== null && frame === null) {
    body = { kind: "kept", groundAlbedo: numberAt(json, "groundAlbedo") };
  } else {
    throw new Error("an air switch fixture holds a ground albedo or a served frame, one alone");
  }
  stringAt(json, "source");
  return {
    name: stringAt(json, "name"),
    figure: {
      equatorialRadiusM: numberAt(figure, "equatorialRadiusM"),
      polarRadiusM: numberAt(figure, "polarRadiusM"),
    },
    body,
    classMap,
    table: {
      bands: numberAt(table, "bands"),
      slots: numberAt(table, "slots"),
      sunFloor: numberAt(table, "sunFloor"),
      halves: stringAt(table, "halves"),
    },
  };
}

/** A table's values as its fixture's base64 of their half bits, little-endian. */
export function halvesOf(values: Float32Array): string {
  const bytes = new Uint8Array(values.length * 2);
  const view = new DataView(bytes.buffer);
  values.forEach((value, i) => {
    view.setUint16(2 * i, toHalfBits(value), true);
  });
  let text = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(text);
}

/** A fixture's table, its values read back from their half bits. */
export function tableOf(table: AirSwitchTable): DiscReflectanceTable {
  const text = atob(table.halves);
  const view = new DataView(new ArrayBuffer(text.length));
  for (let i = 0; i < text.length; i += 1) {
    view.setUint8(i, text.charCodeAt(i));
  }
  const values = new Float32Array(text.length / 2);
  for (let i = 0; i < values.length; i += 1) {
    values[i] = fromHalfBits(view.getUint16(2 * i, true));
  }
  if (values.length !== table.bands * table.slots * DISC_LAYER_VALUES) {
    throw new Error(`an air switch table of ${values.length} values is not its layers'`);
  }
  return { bands: table.bands, slots: table.slots, sunFloor: table.sunFloor, values };
}

/** A grey Lambert photometry of albedo A: p = A ÷ 1.5 and q = 1.5, as `ATMOSPHERE TEST`'s. */
function lambertPhotometry(albedo: number): BodyPhotometry {
  const p = albedo / 1.5;
  return {
    geometricAlbedo: [p, p, p],
    phaseIntegral: [1.5, 1.5, 1.5],
    law: lawFor([p, p, p], [1.5, 1.5, 1.5], "lambert"),
    bondRatioCheck: null,
    provenance: "modelled",
  };
}

/** A served frame's converted record. */
function servedDetailOf(frame: unknown): ReturnType<typeof detailOf> {
  const message = decodeServerMessage(JSON.stringify(frame));
  if (message.type !== "response" || message.body.kind !== "body_detail") {
    throw new Error("a served air switch frame is a body_detail response");
  }
  const system: SystemIdHex = message.body.record.id.slice(0, 16);
  return detailOf(message.body, system);
}

/** A served frame's photometry, as the client converts its section. */
function servedPhotometryOf(frame: unknown): BodyPhotometry {
  const section = servedDetailOf(frame).record.photometry;
  if (section.state !== "ok") {
    throw new Error("a served air switch world states its photometry");
  }
  return modelledPhotometry(section.value).photometry;
}

/**
 * The synthetic class map: an ocean class and a land class over every texel at R10's 64 a face's
 * side, the land's share a smooth function of the texel's direction, about half the sphere.
 */
export function oceanLandMap(): ClassMapTexels {
  return packClassMap(64, 2, ({ face, i, j }) => {
    const [x, y, z] = faceUvToXyz(face, stToUv((i + 0.5) / 64), stToUv((j + 0.5) / 64));
    const n = Math.hypot(x, y, z);
    const f = sin(3 * (x / n) + 1) * cos(2 * (y / n)) + 0.6 * sin(4 * (z / n));
    const land = Math.min(1, Math.max(0, (f + 0.05) * 4));
    return [1 - land, land];
  });
}

/**
 * The ocean's and the land's Lambert laws: A of 0.045 / 0.06 / 0.083 and 0.18 / 0.15 / 0.105 in r,
 * g and b, about a temperate world's drawn ground, sea and land.
 */
export const OCEAN_LAND_LAWS: readonly [PhotometricLaw, PhotometricLaw] = [
  lawFor([0.03, 0.04, 0.055], [1.5, 1.5, 1.5], "lambert"),
  lawFor([0.12, 0.1, 0.07], [1.5, 1.5, 1.5], "lambert"),
];

/** A case as its disc and its point are drawn, from its fixture. */
export function airSwitchBodyOf(fixture: AirSwitchFixture): AirSwitchBody {
  let photometry: BodyPhotometry;
  switch (fixture.body.kind) {
    case "kept":
      photometry = lambertPhotometry(fixture.body.groundAlbedo);
      break;
    case "served":
      photometry = servedPhotometryOf(fixture.body.frame);
      break;
  }
  const air: DiscAir = {
    table: tableOf(fixture.table),
    texture: { kind: "texture", name: `air switch ${fixture.name}` },
    lnBands: [0],
    gravity: null,
  };
  return {
    name: fixture.name,
    figure: fixture.figure,
    photometry,
    air,
    ...(fixture.classMap
      ? {
          surface: classMapSurface(
            UNREAD_TEXTURES,
            "ocean and land",
            oceanLandMap(),
            OCEAN_LAND_LAWS,
            photometry.law,
          ),
        }
      : {}),
  };
}

/** A sphere case of the reference tracer's as a disc's case, on a body of gravity `gravityMS2`. */
function sphereCaseOf(reference: ReferenceCase, gravityMS2: number): DiscAirCase {
  if (reference.figure.kind !== "sphere" || reference.sources === undefined) {
    throw new Error(`${reference.name} is not a baked sphere case`);
  }
  const bake = sphereBakeBody(reference.figure.radiusM, gravityMS2);
  return {
    name: reference.name,
    medium: reference.medium,
    sources: reference.sources,
    curves: [reference.curves],
    body: { figure: bake.figure, gravity: bake.gravity },
  };
}

/** A served world's case: its medium assembled from its frame as the optics worker assembles it. */
async function servedCaseOf(frame: unknown): Promise<DiscAirCase> {
  const detail = servedDetailOf(frame);
  const photometry = servedPhotometryOf(frame);
  const groundAlbedo: [number, number, number] = [
    photometry.geometricAlbedo[0] * photometry.phaseIntegral[0],
    photometry.geometricAlbedo[1] * photometry.phaseIntegral[1],
    photometry.geometricAlbedo[2] * photometry.phaseIntegral[2],
  ];
  const input = recordInput(detail, undefined, groundAlbedo);
  const kDistributions = await loadKDistributions(
    lineSpeciesOf(input, { envelopes: true }),
    inlinePart,
  );
  const assembled = assembleMedium(input, { envelopes: true, kDistributions });
  if (assembled.medium === undefined) {
    throw new Error(`served world ${detail.record.id} draws no medium`);
  }
  const curved = withCurves(assembled, absorberCurveSlots(assembled.drawn));
  if (curved.medium === undefined) {
    throw new Error(`served world ${detail.record.id} lost its medium to its curves`);
  }
  return {
    name: detail.record.id,
    medium: curved.medium,
    sources: curved.drawn.sources,
    curves: curved.drawn.curves,
    body: curved.drawn.body,
  };
}

/** Mars's surface gravity, m s⁻²: the fit column's (`channels.ts`' `MARS_FIT_COLUMN`). */
const MARS_GRAVITY_MS2 = MARS_FIT_COLUMN.referenceGravityMS2;

/** The fixtures' provenance. */
const SOURCE =
  "R07.T8.a's air bodies at the 3 px switch (plan R07; plan R08, R08.T16.b), baked by " +
  "airSwitchCases.test.ts under HYPERION_AIR_SWITCH=1 and written under HYPERION_BLESS=1 " +
  "(test/airSwitchCases.ts); a served world's frame as a test server's body_detail at full sent " +
  "it at the epoch, Dev Fixture universe, seed 0x4d2, generator version 29, 2026-11-06";

/**
 * A case's fixture, its table baked anew (the module's remarks): a served world's from the frame
 * its committed fixture holds.
 *
 * @param committed - The case's committed fixture, whose frame a served world is baked from.
 * @throws Error for a served world whose committed fixture holds no frame.
 */
export async function bakedAirSwitchFixture(
  name: AirSwitchCaseName,
  committed: AirSwitchFixture | null,
): Promise<AirSwitchFile> {
  let item: DiscAirCase;
  let shown: string;
  let groundAlbedo: number | null = null;
  let frame: unknown = null;
  let classMap = false;
  switch (name) {
    case "test-earth":
      item = {
        name: "earth",
        medium: EARTH_REFERENCE,
        sources: EARTH_REFERENCE_SOURCES,
        curves: EARTH_REFERENCE_CURVES,
        body: earthBody(),
      };
      shown = "TEST EARTH";
      groundAlbedo = EARTH_REFERENCE.groundAlbedo[0];
      break;
    case "test-mars": {
      const mars = REFERENCE_CASES.find((each) => each.name === "mars");
      if (mars === undefined) {
        throw new Error("the reference cases hold mars");
      }
      item = sphereCaseOf(mars, MARS_GRAVITY_MS2);
      shown = "TEST MARS";
      groundAlbedo = mars.medium.groundAlbedo[0];
      break;
    }
    case "test-titan": {
      const titan = titanClassReferenceCase();
      item = sphereCaseOf(titan, TITAN_GM_M3S2 / TITAN_RADIUS_M ** 2);
      shown = "TEST TITAN";
      groundAlbedo = titan.medium.groundAlbedo[0];
      break;
    }
    case "test-venus": {
      const venus = thickReferenceCases()["venus-92bar"];
      item = sphereCaseOf(venus, VENUS_GRAVITY_MS2);
      shown = "TEST VENUS";
      groundAlbedo = venus.medium.groundAlbedo[0];
      break;
    }
    case "served-class-map":
    case "served-decked": {
      frame = committed?.body.kind === "served" ? committed.body.frame : null;
      if (frame === null) {
        throw new Error(`${name}'s committed fixture holds no frame to bake from`);
      }
      item = await servedCaseOf(frame);
      shown = `${item.name} (${name === "served-class-map" ? "class map" : "decked"})`;
      classMap = name === "served-class-map";
      break;
    }
  }
  const { table } = bakedDiscAir(item);
  return {
    source: SOURCE,
    name: shown,
    figure: {
      equatorialRadiusM: item.body.figure.equatorialRadiusM,
      polarRadiusM: item.body.figure.polarRadiusM,
    },
    groundAlbedo,
    frame,
    classMap,
    table: {
      bands: table.bands,
      slots: table.slots,
      sunFloor: table.sunFloor,
      halves: halvesOf(table.values),
    },
  };
}
