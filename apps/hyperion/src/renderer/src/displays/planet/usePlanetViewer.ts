/**
 * The `PLANET VIEWER`'s state (plan R15): the inputs on its panel, its commands, and the planet it
 * shows as `VIEW`'s body inspection.
 *
 * @remarks
 * - **`GENERATE`** asks `viewer_planet` for the inputs, then `system_bodies` for its system at the
 *   epoch, and opens the planet as an inspection: the same kept scene of served bodies `VIEW`'s
 *   `VIEW BODY` opens (`inspectionScene`), with its whole body surveyed in training's scratch log as
 *   it opens (`surveyInspected`). Its sky is the one every generated planet shares
 *   (`ServedSky`'s `shared`), so that the next planet keeps the stars drawn.
 * - **`RANDOMIZE`** asks `viewer_randomize` with the draw seed, puts the planet's inputs it draws
 *   on the panel, and generates them; once that planet is shown, and only then, the seed is the
 *   last draw's and the draw seed moves on by one, so that each press draws another planet.
 * - **`OPEN BODY`** shows a planet of the galaxy itself (plan R15, T5): by its body ID, asking
 *   `viewer_inputs` for its inputs, then `resolve_system` and `system_bodies` for its system at
 *   the epoch; or as `SYSTEM`'s or `VIEW`'s `OPEN IN PLANET VIEWER` hands it over, an inspection
 *   already read, at that display's time. Its inputs, read back, go on the panel, so that
 *   `GENERATE` makes a planet of them, whose other draws are its own; the planet shown is the
 *   galaxy's.
 * - **A moon** opens as the galaxy's own body too, from `SYSTEM`'s or `VIEW`'s hand-over or by its
 *   ID, but has none of the viewer's inputs: the hand-over asks no `viewer_inputs` of it, and by its
 *   ID the server's refusal of it is read as no inputs once its system's bodies say it is a moon.
 *   The panel then keeps its last inputs and holds `GENERATE` and `SHARE`'s string back
 *   (`noInputs`) until an input is entered, a string loaded or another planet shown.
 * - **A command** shows `PENDING` and then its outcome, closed-loop as the guide asks: the planet
 *   shown, or the server's refusal, which names the input it is of, or its failure or time-out. A
 *   newer command cancels the one in flight, and so does leaving the display mounted no longer.
 *   Each request is cancelled after `REQUEST_TIMEOUT_MS` unanswered.
 * - **The planet shown** stays while a newer command is pending or has failed, so that the view
 *   keeps drawing it.
 */
import {
  type BodyIdHex,
  parseBodyId,
  type RequestClient,
  type RequestFailure,
  type RequestKind,
  type RequestOf,
  type ResponseFor,
  type SeedHex,
  type SystemIdHex,
  type UniverseIdHex,
  type UniverseTime,
} from "@hyperion/protocol";
import { useCallback, useEffect, useRef, useState } from "react";

import type { SystemPlace } from "../../lib/scene/model";
import type { BodyKind } from "../../lib/system/model";
import { toBodiesRequest, toSystemBodiesModel } from "../../lib/system/bodiesWire";
import { REQUEST_TIMEOUT_MS, type RequestState } from "../../lib/useServerRequest";
import type { BodyInspection } from "../../view/scenes/inspectionScene";
import { surveyInspected } from "../../view/scenes/inspectionSurvey";
import type { InspectionOpening } from "../view/ViewSceneProvider";
import { ORBITAL_SUN, type SunHolder, type SunSetting, viewerScene } from "./viewerScene";
import {
  DEFAULT_INPUTS,
  fromInputsDto,
  type NumericInputName,
  type PlanetInputs,
  toInputsDto,
} from "./planetInputs";

/** The epoch, the time every planet is opened at, as the server's records are read. */
const EPOCH: UniverseTime = { seconds: 0, nanos: 0 };

/** The viewer's commands. */
export type ViewerCommandKind = "generate" | "randomize" | "open";

/** Where a command stands: pending, its planet shown, or how it failed, as a request's states. */
export type CommandState =
  | { readonly kind: "pending" }
  | { readonly kind: "shown" }
  | Extract<
      RequestState<RequestKind>,
      | { readonly kind: "rejected" }
      | { readonly kind: "timed_out" }
      | { readonly kind: "link_down" }
    >;

/** A command and where it stands, with the input a refusal names, if any. */
export interface ViewerCommand {
  readonly kind: ViewerCommandKind;
  /** `pending`, then its outcome: `shown` once its planet is shown. */
  readonly state: CommandState;
  /** The request field a refusal names (`inputs.semi_major_axis_au`, `draw_seed`), or `null`. */
  readonly field: string | null;
}

/**
 * Where the planet shown comes from, with its inputs as the server read them: generated from the
 * inputs; the galaxy's own, its inputs read back, with whether its system has more stars than the
 * inputs' one host; or a moon of the galaxy, which has no inputs to read back.
 */
export type ShownSource =
  | { readonly kind: "generated"; readonly inputs: PlanetInputs }
  | { readonly kind: "body"; readonly inputs: PlanetInputs; readonly multipleStars: boolean }
  | { readonly kind: "moon" };

/** The inputs of the body `source` shows, as the server read them, or `null` for a moon's. */
export function shownInputs(source: ShownSource): PlanetInputs | null {
  let inputs: PlanetInputs | null = null;
  switch (source.kind) {
    case "generated":
    case "body":
      inputs = source.inputs;
      break;
    case "moon":
      break;
  }
  return inputs;
}

/** The planet the viewer shows: the inspection it opened, and what it was made from. */
export interface ShownPlanet {
  /** The planet: a viewer system's, or the galaxy's. */
  readonly body: BodyIdHex;
  /** Its system: at the Sun-like point, or where the galaxy's stands. */
  readonly place: SystemPlace;
  /** Where it comes from, and its inputs. */
  readonly source: ShownSource;
  /** What the generator drew for each input left to it, and where the planet formed. */
  readonly drawn: Readonly<Partial<Record<NumericInputName, number | null>>>;
  /** Its opening as `VIEW`'s body inspection. */
  readonly opening: InspectionOpening;
}

/** What {@link usePlanetViewer} holds and offers. */
export interface PlanetViewer {
  readonly inputs: PlanetInputs;
  /** Sets one input's wire value, or `null` to leave a drawable one to its draw. */
  readonly setInput: (name: NumericInputName, wire: number | null) => void;
  readonly setSeed: (seed: SeedHex) => void;
  /** Puts a whole set of inputs on the panel, as `LOAD` does from a string. */
  readonly setInputs: (inputs: PlanetInputs) => void;
  /** The last command and where it stands, or `null` before the first. */
  readonly command: ViewerCommand | null;
  /** The planet on show, or `null` before the first. */
  readonly shown: ShownPlanet | null;
  /**
   * Whether the panel holds no inputs of the body on show: a moon is shown and no input has been
   * entered since, so that `GENERATE` and `SHARE` are held back.
   */
  readonly noInputs: boolean;
  /** Generates the panel's inputs. */
  readonly generate: () => void;
  /**
   * Draws a real planet's inputs with {@link PlanetViewer.drawSeed}, then generates them; once the
   * planet is shown, the seed becomes {@link PlanetViewer.lastDraw} and the draw seed moves on.
   */
  readonly randomize: () => void;
  /** The seed `RANDOMIZE` draws with next: 1 as the display mounts. */
  readonly drawSeed: SeedHex;
  readonly setDrawSeed: (seed: SeedHex) => void;
  /** The draw seed of the last draw whose planet was shown, or `null` before the first. */
  readonly lastDraw: SeedHex | null;
  /** Shows a planet of the galaxy by its ID, its inputs read back onto the panel. */
  readonly openBody: (body: BodyIdHex) => void;
  /** Where the planet's sun stands (`viewerScene.ts`): `ORBITAL` as the display mounts. */
  readonly sun: SunSetting;
  /** Sets the sun, which moves the light at the view's next frame, the scene kept. */
  readonly setSun: (sun: SunSetting) => void;
}

/**
 * One hand-over of a planet to the viewer, from `SYSTEM`'s or `VIEW`'s `OPEN IN PLANET VIEWER`
 * (plan R15, T5): its inspection, and how many came before, so that each is opened once.
 */
export interface PlanetViewerOpening {
  readonly inspection: BodyInspection;
  /** Counts the hand-overs from 1. */
  readonly sequence: number;
}

/** What the generator drew of a planet whose every input was read back: nothing. */
const NONE_DRAWN: ShownPlanet["drawn"] = {};

/** The draw seed `RANDOMIZE` starts from as the display mounts. */
const FIRST_DRAW_SEED: SeedHex = "0000000000000001";

/** The draw seed after `seed`: one on, wrapping at 2⁶⁴, in the wire's 16 digits. */
export function nextDrawSeed(seed: SeedHex): SeedHex {
  const next = (BigInt(`0x${seed}`) + 1n) % (1n << 64n);
  return next.toString(16).padStart(16, "0");
}

/** How one request ended. */
type Asked<K extends RequestKind> =
  | { readonly kind: "ok"; readonly response: ResponseFor<K> }
  | { readonly kind: "failed"; readonly failure: RequestFailure }
  | { readonly kind: "timed_out" }
  | { readonly kind: "aborted" };

/** Asks `body`, cancelling it once `signal` aborts or it is unanswered after the time-out. */
async function asked<K extends RequestKind>(
  requests: Pick<RequestClient, "request">,
  body: RequestOf<K>,
  signal: AbortSignal,
): Promise<Asked<K>> {
  if (signal.aborted) {
    return { kind: "aborted" };
  }
  const pending = requests.request(body);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let onAbort: (() => void) | undefined;
  const cut = new Promise<Asked<K>>((resolve) => {
    timer = setTimeout(() => {
      pending.cancel();
      resolve({ kind: "timed_out" });
    }, REQUEST_TIMEOUT_MS);
    onAbort = (): void => {
      pending.cancel();
      resolve({ kind: "aborted" });
    };
    signal.addEventListener("abort", onAbort, { once: true });
  });
  const answered = pending.outcome.then((outcome): Asked<K> =>
    outcome.ok
      ? { kind: "ok", response: outcome.response }
      : { kind: "failed", failure: outcome.error },
  );
  try {
    return await Promise.race([answered, cut]);
  } finally {
    clearTimeout(timer);
    if (onAbort !== undefined) {
      signal.removeEventListener("abort", onAbort);
    }
  }
}

/** A failed request as a command's state, with the field it names. */
function failedCommand(
  kind: ViewerCommandKind,
  outcome: Exclude<Asked<RequestKind>, { readonly kind: "ok" } | { readonly kind: "aborted" }>,
): ViewerCommand {
  if (outcome.kind === "timed_out") {
    return { kind, state: { kind: "timed_out" }, field: null };
  }
  const { failure } = outcome;
  if (failure.code === "link_lost") {
    return { kind, state: { kind: "link_down", reason: failure.message }, field: null };
  }
  // The client's own cancellations never reach here (`asked` reads them as aborted), nor does a
  // superseded request, which this hook never makes; a protocol violation is the server's fault.
  const code =
    failure.code === "aborted" ||
    failure.code === "superseded" ||
    failure.code === "protocol_violation"
      ? "internal"
      : failure.code;
  return {
    kind,
    state: { kind: "rejected", code, reason: failure.message },
    field: "field" in failure ? failure.field : null,
  };
}

/**
 * The inspection's kept scene of `inspection`, or why it cannot be drawn: a planet the generator
 * destroyed, or one not present at the epoch, has no body to draw (`inspectionScene` throws).
 */
function openedOption(
  inspection: BodyInspection,
  sun: SunHolder,
): InspectionOpening["option"] | string {
  try {
    const kept = viewerScene(inspection, sun);
    return { name: kept.name, make: () => kept };
  } catch (error: unknown) {
    const reason = error instanceof Error ? error.message : String(error);
    return `the planet is not drawn at the epoch: ${reason}`;
  }
}

/** The kind of `inspection`'s body as its system's bodies give it, or `undefined`. */
function inspectedKind(inspection: BodyInspection): BodyKind["kind"] | undefined {
  return inspection.bodies.bodies.find((body) => body.id === inspection.body)?.kind.kind;
}

/**
 * Whether the viewer opens `inspection`'s body (plan R15, T5): a planet or a dwarf planet, whose
 * inputs the server reads back, or a moon, shown with none.
 */
export function opensInViewer(inspection: BodyInspection): boolean {
  const kind = inspectedKind(inspection);
  return kind === "planet" || kind === "dwarf_planet" || kind === "moon";
}

/** Whether `inspection`'s body is a moon, which has none of the viewer's inputs. */
function isMoon(inspection: BodyInspection): boolean {
  return inspectedKind(inspection) === "moon";
}

/** A moon opened as the galaxy's own body, with no inputs. */
function openedMoon(inspection: BodyInspection): Opened {
  return { kind: "opened", inspection, drawn: NONE_DRAWN, source: { kind: "moon" } };
}

/** The viewer system's designation: `VIEWER` and its ID's last six digits, upper case. */
export function viewerDesignation(system: SystemIdHex): string {
  return `VIEWER ${system.slice(-6).toUpperCase()}`;
}

/** How a command's opening ended: its planet shown, its failure reported, or cut short. */
type Presented = "shown" | "reported" | "aborted";

/** What opening a planet gave: its inspection and drawn values, or a failed command. */
type Opened =
  | {
      readonly kind: "opened";
      readonly inspection: BodyInspection;
      readonly drawn: ShownPlanet["drawn"];
      readonly source: ShownSource;
    }
  | { readonly kind: "failed"; readonly command: ViewerCommand }
  | { readonly kind: "aborted" };

/** An opening that showed nothing: a failed command, or one cut short. */
type NotOpened = Exclude<Opened, { readonly kind: "opened" }>;

/** A client-side fault reading the server's answer, as a command refused `internal`. */
function faulted(kind: ViewerCommandKind, fault: string): NotOpened {
  return {
    kind: "failed",
    command: { kind, state: { kind: "rejected", code: "internal", reason: fault }, field: null },
  };
}

/**
 * Reads `place`'s system of `universe` at the epoch into an inspection of `body`, or how that
 * failed.
 */
async function inspectionAt(
  requests: Pick<RequestClient, "request">,
  universe: UniverseIdHex,
  place: SystemPlace,
  body: BodyIdHex,
  kind: ViewerCommandKind,
  signal: AbortSignal,
): Promise<{ readonly kind: "read"; readonly inspection: BodyInspection } | NotOpened> {
  const bodies = await asked<"system_bodies">(
    requests,
    toBodiesRequest(universe, place.system, EPOCH),
    signal,
  );
  if (bodies.kind === "aborted") {
    return bodies;
  }
  if (bodies.kind !== "ok") {
    return { kind: "failed", command: failedCommand(kind, bodies) };
  }
  const read = toSystemBodiesModel(bodies.response, place.designation);
  if (read.kind === "fault") {
    return faulted(kind, read.fault);
  }
  return {
    kind: "read",
    inspection: { universe, place, model: read.model, bodies: read.bodies, body, time: EPOCH },
  };
}

/** Reads `body`'s inputs back for `inspection`, its own, as the viewer opens it. */
async function readBack(
  requests: Pick<RequestClient, "request">,
  inspection: BodyInspection,
  signal: AbortSignal,
): Promise<Opened> {
  if (isMoon(inspection)) {
    return openedMoon(inspection);
  }
  const read = await asked<"viewer_inputs">(
    requests,
    { kind: "viewer_inputs", universe: inspection.universe, body: inspection.body },
    signal,
  );
  if (read.kind === "aborted") {
    return read;
  }
  if (read.kind !== "ok") {
    return { kind: "failed", command: failedCommand("open", read) };
  }
  return {
    kind: "opened",
    inspection,
    drawn: NONE_DRAWN,
    source: {
      kind: "body",
      inputs: fromInputsDto(read.response.inputs),
      multipleStars: read.response.multiple_stars,
    },
  };
}

/**
 * Whether `outcome` is the server's refusal of `viewer_inputs` for a body that is no planet:
 * `bad_request` on the field `body` (the server's `viewer::inputs`).
 */
function refusedAsNoPlanet(outcome: Asked<"viewer_inputs">): boolean {
  return (
    outcome.kind === "failed" &&
    outcome.failure.code === "bad_request" &&
    "field" in outcome.failure &&
    outcome.failure.field === "body"
  );
}

/**
 * Reads the galaxy's planet `body` of `universe`: its inputs, then its system's record and its
 * bodies at the epoch; or, the inputs refused as no planet's, a moon with none.
 */
async function galaxyPlanet(
  requests: Pick<RequestClient, "request">,
  universe: UniverseIdHex,
  body: BodyIdHex,
  signal: AbortSignal,
): Promise<Opened> {
  // The inputs first, as a planet's path has always asked them; a body refused as no planet may
  // still be a moon, which its system's bodies tell.
  const probe = await asked<"viewer_inputs">(
    requests,
    { kind: "viewer_inputs", universe, body },
    signal,
  );
  if (probe.kind === "aborted") {
    return probe;
  }
  const noPlanet = refusedAsNoPlanet(probe);
  if (probe.kind !== "ok" && !noPlanet) {
    return { kind: "failed", command: failedCommand("open", probe) };
  }
  const { system } = parseBodyId(body);
  const resolved = await asked<"resolve_system">(
    requests,
    { kind: "resolve_system", universe, system, time: EPOCH },
    signal,
  );
  if (resolved.kind === "aborted") {
    return resolved;
  }
  if (resolved.kind !== "ok") {
    return { kind: "failed", command: failedCommand("open", resolved) };
  }
  const { record } = resolved.response;
  const place: SystemPlace = {
    kind: "charted",
    system,
    designation: record.designation,
    barycentre: record.position,
  };
  const read = await inspectionAt(requests, universe, place, body, "open", signal);
  if (read.kind !== "read") {
    return read;
  }
  if (probe.kind !== "ok") {
    // Refused as no planet: a moon opens with no inputs; anything else is refused as the server
    // refused it.
    return isMoon(read.inspection)
      ? openedMoon(read.inspection)
      : { kind: "failed", command: failedCommand("open", probe) };
  }
  return {
    kind: "opened",
    inspection: read.inspection,
    drawn: NONE_DRAWN,
    source: {
      kind: "body",
      inputs: fromInputsDto(probe.response.inputs),
      multipleStars: probe.response.multiple_stars,
    },
  };
}

/**
 * Generates `inputs` in `universe` and reads the planet's system at the epoch into an inspection.
 */
async function generated(
  requests: Pick<RequestClient, "request">,
  universe: UniverseIdHex,
  inputs: PlanetInputs,
  kind: ViewerCommandKind,
  signal: AbortSignal,
): Promise<Opened> {
  const planet = await asked<"viewer_planet">(
    requests,
    { kind: "viewer_planet", universe, inputs: toInputsDto(inputs) },
    signal,
  );
  if (planet.kind === "aborted") {
    return planet;
  }
  if (planet.kind !== "ok") {
    return { kind: "failed", command: failedCommand(kind, planet) };
  }
  const { system, body, barycentre } = planet.response;
  const place: SystemPlace = {
    kind: "charted",
    system,
    designation: viewerDesignation(system),
    barycentre,
  };
  const read = await inspectionAt(requests, universe, place, body, kind, signal);
  if (read.kind !== "read") {
    return read;
  }
  const { drawn } = planet.response;
  return {
    kind: "opened",
    // Every viewer system stands at one point and is no record of the galaxy: its sky is the one
    // every generated planet shares, asked with no system left out (plan R15; `ServedSky`).
    inspection: { ...read.inspection, sky: "shared" },
    source: { kind: "generated", inputs: fromInputsDto(planet.response.inputs) },
    drawn: {
      formedAtAu: drawn.formed_at_au,
      dayHours: drawn.day_hours,
      tiltDeg: drawn.tilt_deg,
      waterRank: drawn.water_rank,
      carbonRank: drawn.carbon_rank,
      nitrogenRank: drawn.nitrogen_rank,
    },
  };
}

/**
 * The `PLANET VIEWER`'s state (the module's remarks), asking through `requests` in `universe`, the
 * open universe, or asking nothing where it is `null`.
 */
export function usePlanetViewer(
  requests: Pick<RequestClient, "request">,
  universe: UniverseIdHex | null,
  opening: PlanetViewerOpening | null,
): PlanetViewer {
  const [inputs, setInputsState] = useState<PlanetInputs>(DEFAULT_INPUTS);
  const [command, setCommand] = useState<ViewerCommand | null>(null);
  const [shown, setShown] = useState<ShownPlanet | null>(null);
  const [noInputs, setNoInputs] = useState(false);
  // Whether an input has been entered since the command in flight started.
  const entered = useRef(false);
  const [drawSeed, setDrawSeed] = useState<SeedHex>(FIRST_DRAW_SEED);
  const [sun, setSunState] = useState<SunSetting>(ORBITAL_SUN);
  // The scene reads the sun at every frame from this holder, which `setSun` writes with the state.
  const sunHolder = useRef<SunHolder>({ current: ORBITAL_SUN });
  const [lastDraw, setLastDraw] = useState<SeedHex | null>(null);
  // The command in flight, whose controller a newer command and the unmount abort.
  const flight = useRef<AbortController | null>(null);
  useEffect(
    () => () => {
      flight.current?.abort();
    },
    [],
  );

  const start = useCallback((): AbortSignal => {
    flight.current?.abort();
    const controller = new AbortController();
    flight.current = controller;
    entered.current = false;
    return controller.signal;
  }, []);

  /**
   * Shows what `reading` gave for `kind`, reporting the outcome unless `signal` has aborted: how it
   * ended, its planet shown, its failure reported, or cut short with nothing reported.
   */
  const present = useCallback(
    async (
      kind: ViewerCommandKind,
      reading: Promise<Opened>,
      signal: AbortSignal,
    ): Promise<Presented> => {
      const outcome = await reading;
      if (signal.aborted || outcome.kind === "aborted") {
        return "aborted";
      }
      if (outcome.kind === "failed") {
        setCommand(outcome.command);
        return "reported";
      }
      const option = openedOption(outcome.inspection, sunHolder.current);
      if (typeof option === "string") {
        setCommand({
          kind,
          state: { kind: "rejected", code: "bad_request", reason: option },
          field: null,
        });
        return "reported";
      }
      const survey = surveyInspected(requests, outcome.inspection).then((settled) => {
        if (settled.kind === "refused") {
          console.warn(`the viewer's survey of ${settled.body} was refused: ${settled.reason}`);
        }
        return settled;
      });
      // A moon keeps the panel's last inputs, which are none of its own, unless an input was
      // entered while it opened, which gives the panel inputs of its own.
      const read = shownInputs(outcome.source);
      if (read === null) {
        setNoInputs(!entered.current);
      } else {
        setInputsState(read);
        setNoInputs(false);
      }
      setShown((previous) => ({
        body: outcome.inspection.body,
        place: outcome.inspection.place,
        source: outcome.source,
        drawn: outcome.drawn,
        opening: { option, sequence: (previous?.opening.sequence ?? 0) + 1, survey },
      }));
      setCommand({ kind, state: { kind: "shown" }, field: null });
      return "shown";
    },
    [requests],
  );

  /** Opens `inputs`' planet for `kind`; whether it was shown. */
  const open = useCallback(
    async (
      kind: ViewerCommandKind,
      asking: UniverseIdHex,
      given: PlanetInputs,
      signal: AbortSignal,
    ): Promise<boolean> =>
      (await present(kind, generated(requests, asking, given, kind, signal), signal)) === "shown",
    [requests, present],
  );

  /** Reports a fault of the client's own while `signal` still stands, so no command stays pending. */
  const failed = useCallback(
    (kind: ViewerCommandKind, signal: AbortSignal, error: unknown): void => {
      console.error(`the viewer's ${kind} could not be completed:`, error);
      if (!signal.aborted) {
        const reason = error instanceof Error ? error.message : String(error);
        setCommand({ kind, state: { kind: "rejected", code: "internal", reason }, field: null });
      }
    },
    [],
  );

  const generate = useCallback((): void => {
    if (universe === null || noInputs) {
      return;
    }
    const signal = start();
    setCommand({ kind: "generate", state: { kind: "pending" }, field: null });
    open("generate", universe, inputs, signal).catch((error: unknown) => {
      failed("generate", signal, error);
    });
  }, [universe, inputs, noInputs, start, open, failed]);

  const randomize = useCallback((): void => {
    if (universe === null) {
      return;
    }
    const seed = drawSeed;
    const signal = start();
    setCommand({ kind: "randomize", state: { kind: "pending" }, field: null });
    const run = async (): Promise<void> => {
      const draw = await asked<"viewer_randomize">(
        requests,
        { kind: "viewer_randomize", universe, draw_seed: seed },
        signal,
      );
      if (draw.kind === "aborted" || signal.aborted) {
        return;
      }
      if (draw.kind !== "ok") {
        setCommand(failedCommand("randomize", draw));
        return;
      }
      const drawnInputs = fromInputsDto(draw.response.inputs);
      setInputsState(drawnInputs);
      setNoInputs(false);
      if (await open("randomize", universe, drawnInputs, signal)) {
        setLastDraw(seed);
        setDrawSeed(nextDrawSeed(seed));
      }
    };
    run().catch((error: unknown) => {
      failed("randomize", signal, error);
    });
  }, [universe, drawSeed, requests, start, open, failed]);

  const openBody = useCallback(
    (body: BodyIdHex): void => {
      if (universe === null) {
        return;
      }
      const signal = start();
      setCommand({ kind: "open", state: { kind: "pending" }, field: null });
      present("open", galaxyPlanet(requests, universe, body, signal), signal).catch(
        (error: unknown) => {
          failed("open", signal, error);
        },
      );
    },
    [universe, requests, start, present, failed],
  );

  // A planet handed over by `OPEN IN PLANET VIEWER` is opened once: once its outcome is reported,
  // or a newer command has cut it short; one cut short by the display hidden is opened when it is
  // next shown.
  const handedOver = useRef(0);
  useEffect(() => {
    if (opening === null || opening.sequence <= handedOver.current) {
      return undefined;
    }
    let hidden = false;
    const signal = start();
    setCommand({ kind: "open", state: { kind: "pending" }, field: null });
    present("open", readBack(requests, opening.inspection, signal), signal)
      .then((presented) => {
        if (presented !== "aborted" || !hidden) {
          handedOver.current = opening.sequence;
        }
        return undefined;
      })
      .catch((error: unknown) => {
        handedOver.current = opening.sequence;
        failed("open", signal, error);
      });
    return () => {
      hidden = true;
      if (flight.current?.signal === signal) {
        flight.current.abort();
      }
    };
  }, [opening, requests, start, present, failed]);

  // An input entered gives the panel inputs of its own again.
  const setInput = useCallback((name: NumericInputName, wire: number | null): void => {
    setInputsState((previous) => ({ ...previous, values: { ...previous.values, [name]: wire } }));
    entered.current = true;
    setNoInputs(false);
  }, []);
  const setSeed = useCallback((seed: SeedHex): void => {
    setInputsState((previous) => ({ ...previous, seed }));
    entered.current = true;
    setNoInputs(false);
  }, []);
  const setInputs = useCallback((next: PlanetInputs): void => {
    setInputsState(next);
    entered.current = true;
    setNoInputs(false);
  }, []);
  const setSun = useCallback((next: SunSetting): void => {
    sunHolder.current.current = next;
    setSunState(next);
  }, []);

  return {
    inputs,
    setInput,
    setSeed,
    setInputs,
    command,
    shown,
    noInputs,
    generate,
    randomize,
    openBody,
    drawSeed,
    setDrawSeed,
    lastDraw,
    sun,
    setSun,
  };
}
