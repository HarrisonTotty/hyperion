/**
 * Plan R07's airless twins over plan R11's lit passes (perf-bodies-pass; the catalogue's
 * `AIRLESS_ENTRIES`): the clouds' layer and volume (R11.T7.b–c) and the far and near seas
 * (R11.T8.b–c), each drawn by its own check through a body's airless tables (`clouds.ts`,
 * `ocean.ts`), run once by their modules as written and once with each material that has an airless
 * twin made as its twin; every texture either run reads back, the same bytes.
 *
 * @remarks
 * `SourceTableEngine` draws such a pass by its twin wherever the body's tables bound are
 * `createAirlessTables`' zeros, a sea whose air the frame does not draw among them. Its swap is
 * checked on the bodies and the lit terrain (`cellPass.ts`, `litTerrain.ts`); here the twins are
 * made in their modules' places from the start, on the engine the page's wraps, so that each check's
 * every draw is by its twin and the two runs differ in their modules alone. A frame's own draws can
 * be swapped so too ({@link byAirlessTwins}): the rocks' frames, forward and by the visibility
 * buffer, are (`terrainFrame.ts`' `checkRocksByIds`).
 */

import { SourceTableEngine } from "../view/atmosphere/sourceTableEngine";
import {
  airlessMaterial,
  BODY_TABLE_DRAW_TEXTURE,
  isAirlessTable,
} from "../view/atmosphere/surfaceLighting";
import { WGSL_CATALOGUE } from "../view/engine/catalogue";
import type {
  DrawItem,
  MaterialHandle,
  RenderEngine,
  WgslMaterialSpec,
} from "../view/engine/types";
import { checkCloudLayerDraw, checkCloudVolumeDraw } from "./clouds";
import { Checks } from "./harness";
import { checkOceanNear, checkOceanPass } from "./ocean";

/** An engine's run of a check: the textures it read back, in order, and the twins it made. */
interface Recording {
  readonly engine: RenderEngine;
  readonly reads: ArrayBuffer[];
  /** The materials made as their airless twins. */
  twins: number;
}

/** The engine's members that make a material from its spec, its first argument. */
const MAKES = new Set<PropertyKey>(["createMaterial", "createMaterialAsync"]);

/**
 * `inner` as its users see it, keeping a copy of every texture read back, and with `byTwins` each
 * material made as its airless twin where it has one.
 */
function recording(inner: RenderEngine, byTwins: boolean): Recording {
  const reads: ArrayBuffer[] = [];
  const record: Recording = {
    reads,
    twins: 0,
    engine: new Proxy(inner, {
      get(target, property): unknown {
        const value: unknown = Reflect.get(target, property, target);
        if (typeof value !== "function") {
          return value;
        }
        if (MAKES.has(property)) {
          return (spec: WgslMaterialSpec, ...rest: unknown[]): unknown => {
            const twin = byTwins ? airlessMaterial(spec) : null;
            if (twin !== null) {
              record.twins += 1;
            }
            return Reflect.apply(value, target, [twin ?? spec, ...rest]);
          };
        }
        if (property === "readTexture") {
          return async (...args: unknown[]): Promise<unknown> => {
            const bytes: unknown = await Reflect.apply(value, target, args);
            if (bytes instanceof ArrayBuffer) {
              reads.push(bytes.slice(0));
            }
            return bytes;
          };
        }
        return (...args: unknown[]): unknown => Reflect.apply(value, target, args);
      },
    }),
  };
  return record;
}

/** Of two runs' read-backs, how many differ, by count or in any byte. */
function readsApart(a: ReadonlyArray<ArrayBuffer>, b: ReadonlyArray<ArrayBuffer>): number {
  let apart = Math.abs(a.length - b.length);
  for (let k = 0; k < Math.min(a.length, b.length); k += 1) {
    const x = new Uint8Array(a[k] ?? new ArrayBuffer(0));
    const y = new Uint8Array(b[k] ?? new ArrayBuffer(0));
    if (x.length !== y.length || x.some((byte, i) => byte !== y[i])) {
      apart += 1;
    }
  }
  return apart;
}

/** The passes checked, each by its own check of plan R11's. */
const PASSES: ReadonlyArray<{
  readonly what: string;
  readonly check: (engine: RenderEngine, checks: Checks) => Promise<void>;
}> = [
  { what: "the cloud layer", check: checkCloudLayerDraw },
  { what: "the cloud volume", check: checkCloudVolumeDraw },
  { what: "the ocean pass's far sea", check: checkOceanPass },
  { what: "the near sea", check: checkOceanNear },
];

/**
 * Plan R07's airless twins of plan R11's lit passes (the module's remarks): each pass's check run as
 * written and by its twins, every read-back the same bytes, and a twin made in each.
 */
export async function checkAirlessPassTwins(engine: RenderEngine, checks: Checks): Promise<void> {
  const inner = engine instanceof SourceTableEngine ? engine.inner : engine;
  for (const { what, check } of PASSES) {
    const asWritten = recording(inner, false);
    const byTwins = recording(inner, true);
    const own = [new Checks(), new Checks()] as const;
    // The runs go in turn, each reading its targets back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    await check(asWritten.engine, own[0]);
    // The twins' run, after the run as written has read its every target back.
    // oxlint-disable-next-line no-await-in-loop
    await check(byTwins.engine, own[1]);
    const apart = readsApart(asWritten.reads, byTwins.reads);
    const passes = own.map((run) => run.list.every((each) => each.pass));
    checks.check(
      `plan R07: ${what} drawn by its airless twins draws its modules' bytes`,
      apart === 0 && asWritten.reads.length > 0 && byTwins.twins > 0,
      `${String(asWritten.reads.length)} read-backs as written, ${String(byTwins.reads.length)} by ${String(byTwins.twins)} twins, ${String(apart)} apart; its own checks pass as written ${String(passes[0])}, by its twins ${String(passes[1])}`,
    );
  }
}

/** The catalogue's materials by name, of which {@link byAirlessTwins} makes the airless twins. */
const CATALOGUE_MATERIALS: ReadonlyMap<string, WgslMaterialSpec> = new Map(
  WGSL_CATALOGUE.flatMap((entry) =>
    entry.kind === "material" ? [[entry.spec.name, entry.spec] as const] : [],
  ),
);

/** A frame's draws by their airless twins ({@link byAirlessTwins}). */
export interface TwinnedDraws {
  /** The draws, each over a body's airless tables by its material's airless twin where it has one. */
  readonly draws: ReadonlyArray<DrawItem>;
  /** How many of them a twin draws. */
  readonly twinned: number;
  /** The twins made, by name. */
  readonly names: ReadonlyArray<string>;
  /** Releases the twins made. */
  readonly release: () => void;
}

/**
 * `draws` with each draw whose body's tables are `createAirlessTables`' zeros, and whose material is
 * the catalogue's and has an airless twin, drawn by that twin, made once on `engine` (the engine the
 * page's wraps, which swaps nothing itself) for its `rgba16float` scene: a frame as written, to draw
 * against its twins' (plan R07, perf-bodies-pass).
 */
export async function byAirlessTwins(
  engine: RenderEngine,
  draws: ReadonlyArray<DrawItem>,
): Promise<TwinnedDraws> {
  const made = new Map<string, MaterialHandle>();
  const swapped: DrawItem[] = [];
  let twinned = 0;
  for (const draw of draws) {
    const spec = CATALOGUE_MATERIALS.get(draw.material.name);
    const twin =
      spec === undefined || !isAirlessTable(draw.textures[BODY_TABLE_DRAW_TEXTURE])
        ? null
        : airlessMaterial(spec);
    if (twin === null) {
      swapped.push(draw);
      continue;
    }
    let material = made.get(twin.name);
    if (material === undefined) {
      // Each twin once, in the draws' order.
      // oxlint-disable-next-line no-await-in-loop
      material = await engine.createMaterialAsync(twin, ["rgba16float"], [draw.mesh]);
      made.set(twin.name, material);
    }
    twinned += 1;
    swapped.push({ ...draw, material });
  }
  return {
    draws: swapped,
    twinned,
    names: [...made.keys()],
    release: () => {
      for (const material of made.values()) {
        engine.releaseMaterial(material);
      }
    },
  };
}
