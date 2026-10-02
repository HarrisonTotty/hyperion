/**
 * `just place-ship`: places R03's ship stand-in in a generated system on a running server, so that
 * `VIEW` shows the server's scene instead of its kept scene under `TRAINING`.
 *
 * @remarks
 * Until sessions exist, each open universe's ship stand-in and scene clock are set by the
 * `scene_ship` request alone (rendering plan R03, Design note 2, R03.T6), and the server starts the
 * stand-in at the galactic centre, in no system. No console sends `scene_ship`, so this tool, a
 * client of the server like the bridge, sends it for the developer: it finds or creates a universe,
 * picks a system, and places the ship where the seat camera's default pose looks at something.
 *
 * The seat camera looks along the own ship's nose, and the stand-in has no attitude, so the client
 * gives it one (R02.T17, `standInAttitude` in `view/scene/fromServer.ts`): the nose along the
 * ship's velocity in the system frame, or, at rest, the galactic axes, which put the nose along
 * galactic −z. Hence the two placements:
 *
 * - a body, by default the system's first planet: at rest in the body's own non-rotating frame,
 *   `distance` behind it along its orbital velocity. The ship then moves with the body, its
 *   velocity in the system frame is the body's, and the nose looks along that velocity at the
 *   body. The body's direction of motion turns as it orbits, a degree in a day for an Earth-like
 *   orbit, so a long run drifts off the body.
 * - `barycentre`: at rest in the system frame, `distance` along galactic +z from the barycentre,
 *   so the nose looks back down at it: at a single system's star, but at empty space between the
 *   stars of a wide multiple (the default system's two stars are 19 and 69 au from it).
 *
 * Failures, a refused request or a system with no planet, end the run with a message: they are
 * thrown as `Error`s and `main` reports them, which suits a one-shot command line tool better
 * than result unions through every step.
 *
 * This is a development seam, as the stand-in is: sessions retire both. Nothing it sets is saved,
 * so it is run again after each server start. The server's state is shared, so it moves every
 * client's scene of that universe.
 */

import {
  type BodySummaryDto,
  decodeServerMessage,
  encodeClientMessage,
  galacticDeltaLy,
  galacticPositionFromLy,
  isBodyId,
  isHex64,
  type KinematicsDto,
  RequestClient,
  type RequestKind,
  type RequestOf,
  type ResponseFor,
  type SystemRecord,
  type UniverseInfo,
  type UniverseTime,
} from "@hyperion/protocol";
import { Command, CommanderError, InvalidArgumentError, Option } from "commander";

import {
  DEFAULT_ADDRESS,
  DEFAULT_PORT,
  ENV_SERVER_ADDR,
  ENV_SERVER_PORT,
  parseAddress,
  parsePort,
  serverUrlOf,
} from "../main/cli";

/** The astronomical unit, m (IAU 2012 Resolution B2). */
export const AU_M = 149_597_870_700;

/** The universe used when none is named: created with {@link DEFAULT_SEED} if it does not exist. */
export const DEFAULT_UNIVERSE = "Dev Fixture";

/** The seed a created universe gets unless `--seed` says otherwise: the server tests' `0x4d2`. */
export const DEFAULT_SEED = "00000000000004d2";

/**
 * Where systems are looked for unless `--near` says otherwise, ly in the `GALACTIC` frame: 26,000 ly
 * from the centre in the disc's plane, about the Sun's galactocentric radius (GRAVITY
 * Collaboration 2019, A&A 625, L10: 8.18 kpc). The galaxy's own radius differs per seed; this is
 * only a point in its disc, away from the dense centre where the ship starts.
 */
export const DEFAULT_NEAR_LY: readonly [number, number, number] = [0, 26_000, 0];

/** The radius of the search for a system, ly. */
export const DEFAULT_RADIUS_LY = 40;

/** The most systems examined for one with a planet, nearest first. */
const SYSTEMS_EXAMINED = 60;

/** The default distance from the barycentre, au. */
export const DEFAULT_BARYCENTRE_DISTANCE_AU = 1;

/**
 * The default distance from a body, au: 1.5 million km, inside an Earth's Hill sphere (0.01 au),
 * where a Jupiter spans about 5° and an Earth about half a degree.
 */
export const DEFAULT_BODY_DISTANCE_AU = 0.01;

/** The interval over which a body's velocity is differenced from its two positions, s. */
const VELOCITY_STEP_S = 1;

/** What the seat camera is pointed at. */
export type LookAt =
  | { readonly kind: "barycentre" }
  | { readonly kind: "planet" }
  | { readonly kind: "body"; readonly body: string };

/** The tool's options, after parsing. */
export interface PlaceShipOptions {
  /** The server's address, as a URL host. */
  readonly address: string;
  /** The server's port. */
  readonly port: number;
  /** A universe's name or its ID (16 lowercase hexadecimal digits). */
  readonly universe: string;
  /** The seed of a universe this tool creates, 16 lowercase hexadecimal digits. */
  readonly seed: string;
  /** A system's ID or its designation, or `null` for the nearest system with a planet. */
  readonly system: string | null;
  /** The centre of the search for a system, ly in the `GALACTIC` frame. */
  readonly nearLy: readonly [number, number, number];
  /** The search's radius, ly. */
  readonly radiusLy: number;
  /** What the seat camera looks at. */
  readonly lookAt: LookAt;
  /** The distance from it, au, or `null` for the target's default. */
  readonly distanceAu: number | null;
  /** The scene time the clock is set to, whole seconds from the epoch. */
  readonly timeS: number;
  /** The clock's rate: 0 or a power of ten from 1 to 100,000, which the server checks. */
  readonly rate: number;
}

/** Reads a number that must be finite and above zero. */
function parsePositive(value: string): number {
  const number = Number(value);
  if (value.trim().length === 0 || !Number.isFinite(number) || number <= 0) {
    throw new InvalidArgumentError("expected a number above zero");
  }
  return number;
}

/** Reads a whole number of seconds, or a rate. */
function parseWhole(value: string): number {
  const number = /^-?\d+$/.test(value) ? Number(value) : Number.NaN;
  if (!Number.isSafeInteger(number)) {
    throw new InvalidArgumentError("expected a whole number");
  }
  return number;
}

/** Reads `x,y,z` in light-years. */
function parseNear(value: string): readonly [number, number, number] {
  const parts = value
    .split(",")
    .map((part) => (part.trim().length > 0 ? Number(part) : Number.NaN));
  const [x, y, z] = parts;
  if (
    parts.length !== 3 ||
    x === undefined ||
    y === undefined ||
    z === undefined ||
    !parts.every(Number.isFinite)
  ) {
    throw new InvalidArgumentError("expected x,y,z in light-years, such as 0,26000,0");
  }
  return [x, y, z];
}

/** Reads `--seed`: 1 to 16 hexadecimal digits, written as the wire's 16 lowercase ones. */
function parseSeed(value: string): string {
  if (!/^[0-9a-fA-F]{1,16}$/.test(value)) {
    throw new InvalidArgumentError("expected 1 to 16 hexadecimal digits");
  }
  return value.toLowerCase().padStart(16, "0");
}

/** Reads `--look-at`: `planet`, `barycentre` or a body ID. */
function parseLookAt(value: string): LookAt {
  if (value === "barycentre" || value === "planet") {
    return { kind: value };
  }
  if (isBodyId(value)) {
    return { kind: "body", body: value };
  }
  throw new InvalidArgumentError(
    "expected planet, barycentre or a body ID such as 61ffd967fe000023.0100",
  );
}

/**
 * The tool's command line.
 *
 * @remarks
 * Errors and `--help` throw a `CommanderError` once their text is written, as the client's command
 * line does, so the caller decides how the process ends.
 */
export function buildCommand(): Command {
  return new Command()
    .name("place-ship")
    .description(
      "Places the ship stand-in in a generated system on a running hyperion-server (scene_ship), " +
        "so that VIEW draws the server's scene. Run it again after each server start.",
    )
    .allowExcessArguments(false)
    .showHelpAfterError()
    .addOption(
      new Option("-a, --address <HOST>", "IP address or host name of the server")
        .default(DEFAULT_ADDRESS)
        .env(ENV_SERVER_ADDR)
        .argParser(parseAddress),
    )
    .addOption(
      new Option("-p, --port <PORT>", "port the server listens on")
        .default(DEFAULT_PORT)
        .env(ENV_SERVER_PORT)
        .argParser(parsePort),
    )
    .addOption(
      new Option(
        "-u, --universe <NAME|ID>",
        "the universe, by name or ID; a name not found is created with --seed",
      ).default(DEFAULT_UNIVERSE),
    )
    .addOption(
      new Option("--seed <HEX>", "seed of a universe created by this tool")
        .default(DEFAULT_SEED)
        .argParser(parseSeed),
    )
    .addOption(
      new Option(
        "-s, --system <ID|DESIGNATION>",
        "the system, by ID or by designation within the search (default: the nearest system with a planet)",
      ),
    )
    .addOption(
      new Option("--near <X,Y,Z>", "centre of the search for a system, ly, galactic frame")
        .default(DEFAULT_NEAR_LY, DEFAULT_NEAR_LY.join(","))
        .argParser(parseNear),
    )
    .addOption(
      new Option("--radius <LY>", "radius of the search, ly")
        .default(DEFAULT_RADIUS_LY)
        .argParser(parsePositive),
    )
    .addOption(
      new Option(
        "-l, --look-at <TARGET>",
        "what the seat camera looks at: planet (the system's first), barycentre, or a body ID",
      )
        .default({ kind: "planet" }, "planet")
        .argParser(parseLookAt),
    )
    .addOption(
      new Option(
        "-d, --distance <AU>",
        `distance from the target, au (default ${String(DEFAULT_BARYCENTRE_DISTANCE_AU)} from the barycentre, ${String(DEFAULT_BODY_DISTANCE_AU)} from a body)`,
      ).argParser(parsePositive),
    )
    .addOption(
      new Option("-t, --time <S>", "scene time to set the clock to, s from the epoch")
        .default(0)
        .argParser(parseWhole),
    )
    .addOption(
      new Option("-r, --rate <N>", "clock rate: 0 (paused) or 1, 10, ... 100000")
        .default(1)
        .argParser(parseWhole),
    )
    .exitOverride();
}

/** Narrows an option commander has already parsed, naming the broken invariant otherwise. */
function option<T>(value: unknown, is: (value: unknown) => value is T, name: string): T {
  if (!is(value)) {
    throw new Error(`the command line yielded no ${name}`);
  }
  return value;
}

const isString = (value: unknown): value is string => typeof value === "string";
const isNumber = (value: unknown): value is number => typeof value === "number";
const isOptionalNumber = (value: unknown): value is number | undefined =>
  value === undefined || typeof value === "number";
const isOptionalString = (value: unknown): value is string | undefined =>
  value === undefined || typeof value === "string";
const isNear = (value: unknown): value is readonly [number, number, number] =>
  Array.isArray(value) && value.length === 3 && value.every((part) => typeof part === "number");
const isLookAt = (value: unknown): value is LookAt => {
  if (typeof value !== "object" || value === null || !("kind" in value)) {
    return false;
  }
  if (value.kind === "barycentre" || value.kind === "planet") {
    return true;
  }
  return value.kind === "body" && "body" in value && typeof value.body === "string";
};

/**
 * Parses the arguments the user gave, falling back to the environment and then to the defaults.
 *
 * @throws CommanderError when an argument cannot be used, or when `--help` was asked for.
 */
export function parsePlaceShipArgs(args: readonly string[]): PlaceShipOptions {
  const options = buildCommand()
    .parse([...args], { from: "user" })
    .opts();
  const distance = option(options["distance"], isOptionalNumber, "distance");
  const system = option(options["system"], isOptionalString, "system");
  return {
    address: option(options["address"], isString, "address"),
    port: option(options["port"], isNumber, "port"),
    universe: option(options["universe"], isString, "universe"),
    seed: option(options["seed"], isString, "seed"),
    system: system ?? null,
    nearLy: option(options["near"], isNear, "search centre"),
    radiusLy: option(options["radius"], isNumber, "radius"),
    lookAt: option(options["lookAt"], isLookAt, "target"),
    distanceAu: distance ?? null,
    timeS: option(options["time"], isNumber, "time"),
    rate: option(options["rate"], isNumber, "rate"),
  };
}

/** Sends one request and waits for its answer: the response, or an error naming the failure. */
export type Ask = <K extends RequestKind>(body: RequestOf<K>) => Promise<ResponseFor<K>>;

/**
 * Wraps a {@link RequestClient} as an {@link Ask} that throws on a failed request.
 *
 * @remarks
 * The returned function throws an `Error` naming the request's kind, the failure's code and
 * message, and the field the server refused, if any.
 */
export function askThrough(client: RequestClient): Ask {
  return async (body) => {
    const outcome = await client.request(body).outcome;
    if (!outcome.ok) {
      const field = "field" in outcome.error && outcome.error.field !== null;
      throw new Error(
        `${body.kind} failed: ${outcome.error.code}: ${outcome.error.message}` +
          (field ? ` (field ${String(outcome.error.field)})` : ""),
      );
    }
    return outcome.response;
  };
}

/** What the tool did, for its report. */
export interface Placement {
  /** The universe. */
  readonly universe: UniverseInfo;
  /** Whether this run created it. */
  readonly created: boolean;
  /** The system's ID. */
  readonly system: string;
  /** Its designation, or `null` when it was named by ID and not found in the search. */
  readonly designation: string | null;
  /** The body looked at, or `null` for the barycentre. */
  readonly body: string | null;
  /** The distance from the target, m. */
  readonly distanceM: number;
  /** The stand-in's kinematics as sent. */
  readonly ship: KinematicsDto;
  /** The clock rate as the server set it. */
  readonly rate: number;
}

/** The universe `name` or ID names, created with `seed` when a name is not found. */
async function findUniverse(
  ask: Ask,
  universe: string,
  seed: string,
): Promise<{ readonly info: UniverseInfo; readonly created: boolean }> {
  const { universes } = await ask({ kind: "list_universes" });
  const found = universes.find((info) => info.id === universe || info.name === universe);
  if (found !== undefined) {
    if (found.status !== "compatible") {
      throw new Error(
        `universe ${found.name} (${found.id}) was made by another generator version; name another`,
      );
    }
    return { info: found, created: false };
  }
  if (isHex64(universe)) {
    throw new Error(`no universe has the ID ${universe}`);
  }
  const info = await ask({ kind: "create_universe", name: universe, seed });
  return { info, created: true };
}

/**
 * A system's bodies at `time`, at `mass_and_orbit`: the lowest detail level that states each body's
 * kind (`contact` leaves it `unresolved`), and every level carries the positions.
 */
async function bodiesAt(
  ask: Ask,
  universe: string,
  system: string,
  time: UniverseTime,
): Promise<readonly BodySummaryDto[]> {
  const answer = await ask({
    kind: "system_bodies",
    universe,
    system,
    time,
    detail: "mass_and_orbit",
  });
  return answer.bodies;
}

/** Whether a body can be a frame for the ship and is there to be looked at: a placed world. */
export function isPlacedWorld(body: BodySummaryDto): boolean {
  const world =
    body.kind.type === "planet" || body.kind.type === "dwarf_planet" || body.kind.type === "moon";
  return world && body.state.type === "present" && body.position_m !== null;
}

/** The system's first planet that is present, in index order, or `undefined`. */
export function firstPlanet(bodies: readonly BodySummaryDto[]): BodySummaryDto | undefined {
  return bodies.find((body) => body.kind.type === "planet" && isPlacedWorld(body));
}

/** The systems of a search, nearest to `nearLy` first. */
export function nearestFirst(
  systems: readonly SystemRecord[],
  nearLy: readonly [number, number, number],
): SystemRecord[] {
  const centre = galacticPositionFromLy(nearLy);
  const distance = (system: SystemRecord): number =>
    Math.hypot(...galacticDeltaLy(centre, system.position));
  return systems.toSorted((a, b) => distance(a) - distance(b));
}

/**
 * The stand-in at rest `distanceM` along galactic +z from `system`'s barycentre, so that a nose
 * along galactic −z looks at it.
 */
export function barycentreKinematics(
  system: string,
  distanceM: number,
  time: UniverseTime,
): KinematicsDto {
  return {
    position: { frame: "system", system, offset_m: [0, 0, distanceM] },
    velocity_m_s: [0, 0, 0],
    time,
  };
}

/**
 * The stand-in at rest in `body`'s frame, `distanceM` behind it along its velocity in the system
 * frame, so that a nose along that velocity looks at it.
 *
 * @throws Error if the velocity is zero or not finite, which leaves no direction to look along.
 */
export function bodyKinematics(
  body: string,
  velocityMS: readonly [number, number, number],
  distanceM: number,
  time: UniverseTime,
): KinematicsDto {
  const speed = Math.hypot(...velocityMS);
  if (!Number.isFinite(speed) || speed === 0) {
    throw new Error(`body ${body} has no velocity in its system to look along`);
  }
  const scale = -distanceM / speed;
  return {
    position: {
      frame: "body",
      body,
      offset_m: [velocityMS[0] * scale, velocityMS[1] * scale, velocityMS[2] * scale],
    },
    velocity_m_s: [0, 0, 0],
    time,
  };
}

/** The system named by `options.system`, or the nearest with a planet. */
async function chooseSystem(
  ask: Ask,
  universe: string,
  options: PlaceShipOptions,
  time: UniverseTime,
): Promise<{ readonly id: string; readonly designation: string | null }> {
  if (options.system !== null && isHex64(options.system)) {
    return { id: options.system, designation: null };
  }
  const range = await ask({
    kind: "systems_in_range",
    universe,
    centre: galacticPositionFromLy(options.nearLy),
    radius_ly: options.radiusLy,
    time,
    min_layer: "c",
    limit: 2_000,
  });
  const systems = nearestFirst(range.systems, options.nearLy);
  const wanted = options.system?.toUpperCase();
  if (wanted !== undefined) {
    const named = systems.find((system) => system.designation.toUpperCase() === wanted);
    if (named === undefined) {
      throw new Error(
        `no system designated ${String(options.system)} within ${String(options.radiusLy)} ly of ` +
          `${options.nearLy.join(",")} ly; give its ID, or --near and --radius`,
      );
    }
    return { id: named.id, designation: named.designation };
  }
  for (const system of systems.slice(0, SYSTEMS_EXAMINED)) {
    // One system at a time, nearest first, stopping at the first with a planet: asking for all
    // sixty at once would generate systems that are never used.
    // oxlint-disable-next-line no-await-in-loop
    const bodies = await bodiesAt(ask, universe, system.id, time);
    if (firstPlanet(bodies) !== undefined) {
      return { id: system.id, designation: system.designation };
    }
  }
  throw new Error(
    `none of the ${String(Math.min(systems.length, SYSTEMS_EXAMINED))} systems nearest ` +
      `${options.nearLy.join(",")} ly has a planet; try another --near or a larger --radius`,
  );
}

/** The body to look at, its system-frame velocity differenced from two positions. */
async function bodyTarget(
  ask: Ask,
  universe: string,
  system: string,
  lookAt: Exclude<LookAt, { kind: "barycentre" }>,
  time: UniverseTime,
): Promise<{ readonly id: string; readonly velocityMS: readonly [number, number, number] }> {
  const before = await bodiesAt(ask, universe, system, time);
  const body =
    lookAt.kind === "planet"
      ? firstPlanet(before)
      : before.find((candidate) => candidate.id === lookAt.body);
  if (body === undefined) {
    throw new Error(
      lookAt.kind === "planet"
        ? `system ${system} has no planet present at ${String(time.seconds)} s`
        : `system ${system} has no body ${lookAt.body}`,
    );
  }
  if (!isPlacedWorld(body)) {
    throw new Error(
      `body ${body.id} is not a planet, dwarf planet or moon present at ${String(time.seconds)} s`,
    );
  }
  const later = { seconds: time.seconds + VELOCITY_STEP_S, nanos: time.nanos };
  const after = (await bodiesAt(ask, universe, system, later)).find((next) => next.id === body.id);
  const from = body.position_m;
  const to = after?.position_m;
  if (from === null || to === undefined || to === null) {
    throw new Error(`body ${body.id} has no position ${String(VELOCITY_STEP_S)} s later`);
  }
  return {
    id: body.id,
    velocityMS: [
      (to[0] - from[0]) / VELOCITY_STEP_S,
      (to[1] - from[1]) / VELOCITY_STEP_S,
      (to[2] - from[2]) / VELOCITY_STEP_S,
    ],
  };
}

/**
 * Finds or creates the universe, chooses the system and the target, and sends `scene_ship`.
 *
 * @throws Error naming what is missing: a universe ID that names none, a universe of another
 * generator version, a designation not found in the search, no system with a planet in it, a
 * target that is not a planet, dwarf planet or moon present at the time, or any request the
 * server refused (see {@link askThrough}).
 */
export async function placeShip(ask: Ask, options: PlaceShipOptions): Promise<Placement> {
  const { info, created } = await findUniverse(ask, options.universe, options.seed);
  const time: UniverseTime = { seconds: options.timeS, nanos: 0 };
  const system = await chooseSystem(ask, info.id, options, time);
  let ship: KinematicsDto;
  let body: string | null = null;
  let distanceAu: number;
  if (options.lookAt.kind === "barycentre") {
    distanceAu = options.distanceAu ?? DEFAULT_BARYCENTRE_DISTANCE_AU;
    ship = barycentreKinematics(system.id, distanceAu * AU_M, time);
  } else {
    distanceAu = options.distanceAu ?? DEFAULT_BODY_DISTANCE_AU;
    const target = await bodyTarget(ask, info.id, system.id, options.lookAt, time);
    body = target.id;
    ship = bodyKinematics(target.id, target.velocityMS, distanceAu * AU_M, time);
  }
  const set = await ask({
    kind: "scene_ship",
    universe: info.id,
    ship,
    time_rate: options.rate,
  });
  return {
    universe: info,
    created,
    system: system.id,
    designation: system.designation,
    body,
    distanceM: distanceAu * AU_M,
    ship,
    rate: set.clock.time_rate,
  };
}

/** The report printed once the ship is placed. */
export function describePlacement(placement: Placement): string {
  const target = placement.body === null ? "the barycentre" : `body ${placement.body}`;
  const system =
    placement.designation === null
      ? placement.system
      : `${placement.designation} (${placement.system})`;
  return [
    `universe  ${placement.universe.name} (${placement.universe.id}, seed ${placement.universe.seed})` +
      (placement.created ? ", created" : ""),
    `system    ${system}`,
    `ship      ${String(placement.distanceM / AU_M)} au from ${target}, looking at it from the seat`,
    `clock     ${String(placement.ship.time.seconds)} s, rate ${String(placement.rate)}`,
    `next      in the client: F2, open ${placement.universe.name}, then F4 (VIEW)`,
  ].join("\n");
}

/** How long the tool waits for the server to answer the hello, ms. */
const WELCOME_TIMEOUT_MS = 10_000;

/** A link to the server, welcomed. */
interface Link {
  /** Sends requests on the link. */
  readonly client: RequestClient;
  /** Closes the socket. */
  readonly close: () => void;
}

/**
 * Links to the server at `url`, says hello, and resolves with a request client once welcomed.
 *
 * @throws Error if the socket fails or closes, or no welcome comes within the timeout; the socket
 * is closed first, so that it holds the process open no longer.
 */
async function link(url: string): Promise<Link> {
  const socket = new WebSocket(url);
  socket.binaryType = "arraybuffer";
  let open = false;
  const client = new RequestClient((message) => {
    if (!open) {
      return false;
    }
    socket.send(encodeClientMessage(message));
    return true;
  });
  const welcomed = new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`no welcome from ${url} within ${String(WELCOME_TIMEOUT_MS / 1000)} s`));
    }, WELCOME_TIMEOUT_MS);
    socket.addEventListener("open", () => {
      open = true;
      socket.send(encodeClientMessage({ type: "hello", client_version: "place-ship" }));
    });
    socket.addEventListener("message", (event: MessageEvent) => {
      if (typeof event.data !== "string") {
        return;
      }
      const message = decodeServerMessage(event.data);
      if (message.type === "welcome") {
        clearTimeout(timer);
        resolve();
      } else if (message.type === "error") {
        console.error(`server error: ${message.message}`);
      } else {
        client.handleServerMessage(message);
      }
    });
    socket.addEventListener("error", () => {
      clearTimeout(timer);
      reject(new Error(`could not link to ${url}: is \`just server\` running?`));
    });
    socket.addEventListener("close", () => {
      open = false;
      clearTimeout(timer);
      client.linkLost();
      reject(new Error(`the link to ${url} closed`));
    });
  });
  try {
    await welcomed;
  } catch (error: unknown) {
    socket.close();
    throw error;
  }
  return {
    client,
    close: () => {
      socket.close();
    },
  };
}

/**
 * Runs the tool with the arguments the user gave, writing its report to stdout.
 *
 * @returns The process's exit code.
 */
export async function main(args: readonly string[]): Promise<number> {
  let options: PlaceShipOptions;
  try {
    options = parsePlaceShipArgs(args);
  } catch (error: unknown) {
    if (error instanceof CommanderError) {
      // Commander has written its usage or help; its exit code is on the error.
      return error.exitCode;
    }
    throw error;
  }
  const url = serverUrlOf(options);
  try {
    const { client, close } = await link(url);
    try {
      const placement = await placeShip(askThrough(client), options);
      process.stdout.write(`${describePlacement(placement)}\n`);
    } finally {
      close();
    }
    return 0;
  } catch (error: unknown) {
    console.error(`place-ship: ${error instanceof Error ? error.message : String(error)}`);
    return 1;
  }
}
