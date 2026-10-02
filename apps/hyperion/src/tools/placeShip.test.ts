import {
  type BodySummaryDto,
  type ClientMessage,
  type DetailLevelDto,
  galacticPositionFromLy,
  type RequestBody,
  RequestClient,
  type RequestError,
  PROTOCOL_VERSION,
  type ResponseBody,
  type ServerMessage,
  type SystemRecord,
  type UniverseInfo,
} from "@hyperion/protocol";
import { CommanderError } from "commander";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  AU_M,
  askThrough,
  bodyKinematics,
  DEFAULT_BODY_DISTANCE_AU,
  DEFAULT_NEAR_LY,
  DEFAULT_SEED,
  DEFAULT_UNIVERSE,
  describePlacement,
  nearestFirst,
  type PlaceShipOptions,
  parsePlaceShipArgs,
  main,
  placeShip,
} from "./placeShip";

const UNIVERSE: UniverseInfo = {
  id: "00000000000000aa",
  name: "Survey",
  seed: "00000000000004d2",
  generator_version: 18,
  status: "compatible",
};

const NEAR = "0000000000000001";
const FAR = "0000000000000002";
const EMPTY = "0000000000000003";

function system(id: string, designation: string, offsetLy: number): SystemRecord {
  return {
    id,
    designation,
    position: galacticPositionFromLy([DEFAULT_NEAR_LY[0] + offsetLy, DEFAULT_NEAR_LY[1], 0]),
    layer: "c",
    initial_mass_msun: 1,
    age_myr: 4_600,
    population: "old_thin_disc",
    velocity_km_s: [0, 220, 0],
  };
}

function aBody(
  id: string,
  type: "planet" | "moon" | "ring",
  positionM: [number, number, number] | null,
): BodySummaryDto {
  return {
    id,
    kind: type === "moon" ? { type, origin: "regular" } : { type },
    label: { state: "not_resolved" },
    parent: null,
    state: positionM === null ? { type: "not_yet_formed" } : { type: "present" },
    position_m: positionM,
    mass_kg: { state: "not_resolved" },
    orbit: { state: "not_resolved" },
    moons: { state: "not_resolved" },
    rings: { state: "not_resolved" },
    population: { state: "not_resolved" },
    bulk: { state: "not_resolved" },
  };
}

/** A planet 1 au out on +x, moving at 30 km/s along +y. */
function planetAt(seconds: number): BodySummaryDto {
  return aBody(`${NEAR}.0100`, "planet", [AU_M, 30_000 * seconds, 0]);
}

/**
 * A server answering the tool's requests from fixed data, recording what it was sent.
 *
 * @remarks
 * The systems are listed far first, so that choosing the nearest is the tool's doing; the nearest
 * with planets is `NEAR`, and `EMPTY`, nearer still, has none.
 */
class FakeServer {
  readonly universes: UniverseInfo[] = [UNIVERSE];
  readonly requests: RequestBody[] = [];

  answer(body: RequestBody): ResponseBody | RequestError {
    this.requests.push(body);
    switch (body.kind) {
      case "list_universes":
        return { kind: "list_universes", universes: this.universes, server_generator_version: 18 };
      case "create_universe": {
        const created: UniverseInfo = {
          id: "00000000000000bb",
          name: body.name,
          seed: body.seed ?? "0",
          generator_version: 18,
          status: "compatible",
        };
        this.universes.push(created);
        return { kind: "create_universe", ...created };
      }
      case "systems_in_range":
        return {
          kind: "systems_in_range",
          universe: body.universe,
          centre: body.centre,
          radius_ly: body.radius_ly,
          time: body.time,
          census: { limit: body.limit, complete_above_msun: null, layers: [] },
          systems: [
            system(FAR, "FAR 1", 30),
            system(NEAR, "NEAR 1", 5),
            system(EMPTY, "EMPTY 1", 1),
          ],
        };
      case "system_bodies":
        return {
          kind: "system_bodies",
          granted: body.detail,
          hosts: {
            universe: body.universe,
            system: body.system,
            time: body.time,
            existence: "exists",
            age_myr: 4_600,
            fe_h_dex: 0,
            stars: [],
            hierarchy: { nodes: [{ type: "star", body_index: 0, mass_msun: 1 }] },
          },
          zones: [],
          system_plane: null,
          belts: { state: "ok", value: [] },
          halo: { state: "ok", value: null },
          bodies: body.system === EMPTY ? [] : this.#bodies(body.time.seconds, body.detail),
        };
      case "scene_ship":
        return {
          kind: "scene_ship",
          clock: { time: body.ship.time, time_rate: body.time_rate, state: "running" },
        };
      case "open_universe":
      case "galaxy_parameters":
      case "density_map":
      case "system_summary":
      case "body_detail":
      case "body_events":
      case "subscribe":
      case "unsubscribe":
      case "scene_cameras":
        break;
    }
    return { code: "unsupported", message: `not faked: ${body.kind}`, field: null };
  }

  /** A system's bodies; as the server does, `contact` leaves every body's kind unresolved. */
  #bodies(seconds: number, detail: DetailLevelDto): BodySummaryDto[] {
    const bodies = [
      aBody(`${NEAR}.0001`, "ring", [0, 0, 0]),
      planetAt(seconds),
      aBody(`${NEAR}.0101`, "moon", null),
    ];
    if (detail === "contact") {
      for (const listed of bodies) {
        listed.kind = { type: "unresolved" };
      }
    }
    return bodies;
  }

  /** A request client whose requests this server answers on the next microtask. */
  client(): RequestClient {
    const client = new RequestClient((message: ClientMessage) => {
      if (message.type !== "request") {
        return true;
      }
      const answer = this.answer(message.body);
      queueMicrotask(() => {
        client.handleServerMessage(
          "kind" in answer
            ? { type: "response", id: message.id, body: answer }
            : { type: "request_error", id: message.id, error: answer },
        );
      });
      return true;
    });
    return client;
  }

  /** The `scene_ship` request it was sent. */
  sceneShip(): Extract<RequestBody, { kind: "scene_ship" }> {
    const request = this.requests.find((sent) => sent.kind === "scene_ship");
    if (request?.kind !== "scene_ship") {
      throw new Error("no scene_ship was sent");
    }
    return request;
  }
}

function options(args: readonly string[]): PlaceShipOptions {
  return parsePlaceShipArgs(args);
}

describe("place-ship's command line", () => {
  beforeEach(() => {
    // Commander's usage and help are not printed among the test results.
    for (const stream of [process.stdout, process.stderr]) {
      vi.spyOn(stream, "write").mockReturnValue(true);
    }
    vi.stubEnv("HYPERION_SERVER_ADDR", undefined);
    vi.stubEnv("HYPERION_SERVER_PORT", undefined);
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it("defaults to the dev universe and the first planet of the nearest system with one", () => {
    expect(options([])).toEqual({
      address: "127.0.0.1",
      port: 7878,
      universe: DEFAULT_UNIVERSE,
      seed: DEFAULT_SEED,
      system: null,
      nearLy: DEFAULT_NEAR_LY,
      radiusLy: 40,
      lookAt: { kind: "planet" },
      distanceAu: null,
      timeS: 0,
      rate: 1,
    });
  });

  it("reads the server from the client's variables", () => {
    vi.stubEnv("HYPERION_SERVER_PORT", "17878");
    expect(options([]).port).toBe(17_878);
  });

  it("reads a body to look at, a distance, a search centre and a short seed", () => {
    const parsed = options([
      "--look-at",
      "61ffd967fe000023.0100",
      "--distance",
      "0.05",
      "--near",
      "10,-20.5,3",
      "--seed",
      "4D2",
    ]);
    expect(parsed.lookAt).toEqual({ kind: "body", body: "61ffd967fe000023.0100" });
    expect(parsed.distanceAu).toBe(0.05);
    expect(parsed.nearLy).toEqual([10, -20.5, 3]);
    expect(parsed.seed).toBe("00000000000004d2");
  });

  it.each([
    [["--look-at", "moon"]],
    [["--distance", "0"]],
    [["--near", "1,2"]],
    [["--seed", "xyz"]],
    [["--time", "1.5"]],
    [["stray"]],
  ])("refuses %j", (args) => {
    expect(() => options(args)).toThrow(CommanderError);
  });
});

describe("placing the ship", () => {
  it("places it at rest 1 au along galactic +z from the barycentre when asked", async () => {
    const server = new FakeServer();
    const placement = await placeShip(
      askThrough(server.client()),
      options(["--universe", "Survey", "--time", "3600", "--look-at", "barycentre"]),
    );
    expect(placement).toMatchObject({ created: false, system: NEAR, designation: "NEAR 1" });
    expect(server.sceneShip()).toEqual({
      kind: "scene_ship",
      universe: UNIVERSE.id,
      ship: {
        position: { frame: "system", system: NEAR, offset_m: [0, 0, AU_M] },
        velocity_m_s: [0, 0, 0],
        time: { seconds: 3600, nanos: 0 },
      },
      time_rate: 1,
    });
  });

  it("creates the named universe with the seed when no universe has that name", async () => {
    const server = new FakeServer();
    const placement = await placeShip(askThrough(server.client()), options([]));
    expect(placement.created).toBe(true);
    expect(server.requests).toContainEqual({
      kind: "create_universe",
      name: DEFAULT_UNIVERSE,
      seed: DEFAULT_SEED,
    });
    expect(server.sceneShip().universe).toBe("00000000000000bb");
  });

  it("refuses a universe ID that names none", async () => {
    const server = new FakeServer();
    await expect(
      placeShip(askThrough(server.client()), options(["--universe", "00000000000000cc"])),
    ).rejects.toThrow("no universe has the ID 00000000000000cc");
  });

  it("finds a system by its designation within the search", async () => {
    const server = new FakeServer();
    const placement = await placeShip(
      askThrough(server.client()),
      options(["--universe", "Survey", "--system", "far 1"]),
    );
    expect(placement.system).toBe(FAR);
  });

  it("puts the ship behind the first planet along its velocity, at rest in its frame", async () => {
    const server = new FakeServer();
    const placement = await placeShip(askThrough(server.client()), options(["-u", "Survey"]));
    expect(placement.body).toBe(`${NEAR}.0100`);
    const ship = server.sceneShip().ship;
    expect(ship.position).toEqual({
      frame: "body",
      body: `${NEAR}.0100`,
      offset_m: [-0, -DEFAULT_BODY_DISTANCE_AU * AU_M, -0],
    });
    expect(ship.velocity_m_s).toEqual([0, 0, 0]);
  });

  it("refuses a body that is not there to look at", async () => {
    const server = new FakeServer();
    await expect(
      placeShip(
        askThrough(server.client()),
        options(["--universe", "Survey", "--system", NEAR, "--look-at", `${NEAR}.0101`]),
      ),
    ).rejects.toThrow("is not a planet, dwarf planet or moon present");
  });

  it("reports a server's refusal with its code and field", async () => {
    const server = new FakeServer();
    vi.spyOn(server, "answer").mockReturnValue({
      code: "bad_request",
      message: "no",
      field: "time_rate",
    });
    await expect(placeShip(askThrough(server.client()), options([]))).rejects.toThrow(
      "list_universes failed: bad_request: no (field time_rate)",
    );
  });

  it("tells the developer what to open next", async () => {
    const server = new FakeServer();
    const placement = await placeShip(askThrough(server.client()), options(["-u", "Survey"]));
    expect(describePlacement(placement)).toContain("F2, open Survey, then F4 (VIEW)");
  });
});

describe("the geometry", () => {
  it("orders systems by distance from the search centre", () => {
    const systems = [system(FAR, "FAR", 30), system(NEAR, "NEAR", -5)];
    expect(nearestFirst(systems, DEFAULT_NEAR_LY).map((record) => record.id)).toEqual([NEAR, FAR]);
  });

  it("places the ship against the velocity at the distance asked", () => {
    const kinematics = bodyKinematics("b", [3, 4, 0], 10, { seconds: 0, nanos: 0 });
    expect(kinematics.position).toEqual({ frame: "body", body: "b", offset_m: [-6, -8, -0] });
  });

  it("refuses a body with no velocity to look along", () => {
    expect(() => bodyKinematics("b", [0, 0, 0], 10, { seconds: 0, nanos: 0 })).toThrow(
      "no velocity",
    );
  });
});

/** Whether a message the client sent is a request, as the fake socket recorded it. */
function isRequest(message: unknown): message is { id: number; body: RequestBody } {
  return (
    typeof message === "object" &&
    message !== null &&
    "type" in message &&
    message.type === "request" &&
    "id" in message &&
    typeof message.id === "number" &&
    "body" in message
  );
}

/**
 * A stand-in for Node's `WebSocket`, the boundary the tool links through, whose requests a
 * {@link FakeServer} answers once the test has welcomed the client.
 *
 * @remarks
 * The renderer's `FakeWebSocket` is typed against the DOM library, which the tool's Node project
 * does not load, so the tool's tests keep this smaller one.
 */
class AnsweringSocket extends EventTarget {
  static server = new FakeServer();
  static readonly instances: AnsweringSocket[] = [];
  binaryType = "blob";
  readonly sent: unknown[] = [];
  closed = false;

  constructor(readonly url: string) {
    super();
    AnsweringSocket.instances.push(this);
  }

  static latest(): AnsweringSocket {
    const socket = AnsweringSocket.instances.at(-1);
    if (socket === undefined) {
      throw new Error("no WebSocket has been opened");
    }
    return socket;
  }

  send(data: string): void {
    const message: unknown = JSON.parse(data);
    this.sent.push(message);
    if (!isRequest(message)) {
      return;
    }
    const answer = AnsweringSocket.server.answer(message.body);
    queueMicrotask(() => {
      this.serverSends(
        "kind" in answer
          ? { type: "response", id: message.id, body: answer }
          : { type: "request_error", id: message.id, error: answer },
      );
    });
  }

  close(): void {
    this.closed = true;
    this.dispatchEvent(new Event("close"));
  }

  serverOpens(): void {
    this.dispatchEvent(new Event("open"));
  }

  serverSends(message: ServerMessage): void {
    this.dispatchEvent(new MessageEvent("message", { data: JSON.stringify(message) }));
  }

  serverWelcomes(): void {
    this.serverOpens();
    this.serverSends({
      type: "welcome",
      server_version: "9.9.9",
      protocol_version: PROTOCOL_VERSION,
      generator_version: 18,
    });
  }
}

describe("the tool's run", () => {
  beforeEach(() => {
    AnsweringSocket.instances.length = 0;
    AnsweringSocket.server = new FakeServer();
    vi.stubGlobal("WebSocket", AnsweringSocket);
    vi.spyOn(process.stdout, "write").mockReturnValue(true);
    vi.spyOn(process.stderr, "write").mockReturnValue(true);
    vi.spyOn(console, "error").mockReturnValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("says hello, places the ship, closes the socket and exits 0", async () => {
    const run = main(["-u", "Survey", "--port", "17878"]);
    const socket = AnsweringSocket.latest();
    expect(socket.url).toBe("ws://127.0.0.1:17878/ws");
    socket.serverWelcomes();
    await expect(run).resolves.toBe(0);
    expect(socket.sent[0]).toEqual({ type: "hello", client_version: "place-ship" });
    expect(AnsweringSocket.server.sceneShip().universe).toBe(UNIVERSE.id);
    expect(socket.closed).toBe(true);
  });

  it("exits 1 and closes the socket when the server never welcomes it", async () => {
    vi.useFakeTimers();
    const run = main([]);
    const socket = AnsweringSocket.latest();
    socket.serverOpens();
    await vi.advanceTimersByTimeAsync(10_000);
    await expect(run).resolves.toBe(1);
    expect(socket.closed).toBe(true);
    expect(console.error).toHaveBeenCalledWith(expect.stringContaining("no welcome"));
  });

  it("exits 1 with the reason when the placement fails", async () => {
    const run = main(["-u", "00000000000000cc"]);
    AnsweringSocket.latest().serverWelcomes();
    await expect(run).resolves.toBe(1);
    expect(console.error).toHaveBeenCalledWith(
      "place-ship: no universe has the ID 00000000000000cc",
    );
  });

  it("exits with commander's code for help and a bad argument, opening no socket", async () => {
    await expect(main(["--help"])).resolves.toBe(0);
    await expect(main(["--rate", "x"])).resolves.not.toBe(0);
    expect(AnsweringSocket.instances).toHaveLength(0);
  });
});
