/**
 * The `PLANET VIEWER`'s state (plan R15): the inputs on its panel, its commands, and the planet it
 * shows as `VIEW`'s body inspection.
 *
 * @remarks
 * - **`GENERATE`** asks `viewer_planet` for the inputs, then `system_bodies` for its system at the
 *   epoch, and opens the planet as an inspection: the same kept scene of served bodies `VIEW`'s
 *   `VIEW BODY` opens (`inspectionScene`), with its whole body surveyed in training's scratch log as
 *   it opens (`surveyInspected`).
 * - **`RANDOMIZE`** asks `viewer_randomize` with the draw seed, puts the planet's inputs it draws
 *   on the panel, and generates them; once that planet is shown, and only then, the seed is the
 *   last draw's and the draw seed moves on by one, so that each press draws another planet.
 * - **A command** shows `PENDING` and then its outcome, closed-loop as the guide asks: the planet
 *   shown, or the server's refusal, which names the input it is of, or its failure or time-out. A
 *   newer command cancels the one in flight, and so does leaving the display mounted no longer.
 *   Each request is cancelled after `REQUEST_TIMEOUT_MS` unanswered.
 * - **The planet shown** stays while a newer command is pending or has failed, so that the view
 *   keeps drawing it.
 */
import type {
  BodyIdHex,
  RequestClient,
  RequestFailure,
  RequestKind,
  RequestOf,
  ResponseFor,
  SeedHex,
  SystemIdHex,
  UniverseIdHex,
  UniverseTime,
} from "@hyperion/protocol";
import { useCallback, useEffect, useRef, useState } from "react";

import type { SystemPlace } from "../../lib/scene/model";
import { toBodiesRequest, toSystemBodiesModel } from "../../lib/system/bodiesWire";
import { REQUEST_TIMEOUT_MS, type RequestState } from "../../lib/useServerRequest";
import type { BodyInspection } from "../../view/scenes/inspectionScene";
import { surveyInspected } from "../../view/scenes/inspectionSurvey";
import { type InspectionOpening, inspectionOption } from "../view/ViewSceneProvider";
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
export type ViewerCommandKind = "generate" | "randomize";

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

/** The planet the viewer shows: the inspection it opened, and what it was made from. */
export interface ShownPlanet {
  /** The viewer system's planet. */
  readonly body: BodyIdHex;
  /** Its system, at the Sun-like point. */
  readonly place: SystemPlace;
  /** Its inputs as the server read them. */
  readonly inputs: PlanetInputs;
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
}

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
function openedOption(inspection: BodyInspection): InspectionOpening["option"] | string {
  try {
    return inspectionOption(inspection);
  } catch (error: unknown) {
    const reason = error instanceof Error ? error.message : String(error);
    return `the planet is not drawn at the epoch: ${reason}`;
  }
}

/** The viewer system's designation: `VIEWER` and its ID's last six digits, upper case. */
export function viewerDesignation(system: SystemIdHex): string {
  return `VIEWER ${system.slice(-6).toUpperCase()}`;
}

/** What opening a generated planet gave: its inspection and drawn values, or a failed command. */
type Opened =
  | {
      readonly kind: "opened";
      readonly inspection: BodyInspection;
      readonly inputs: PlanetInputs;
      readonly drawn: ShownPlanet["drawn"];
    }
  | { readonly kind: "failed"; readonly command: ViewerCommand }
  | { readonly kind: "aborted" };

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
  const bodies = await asked<"system_bodies">(
    requests,
    toBodiesRequest(universe, system, EPOCH),
    signal,
  );
  if (bodies.kind === "aborted") {
    return bodies;
  }
  if (bodies.kind !== "ok") {
    return { kind: "failed", command: failedCommand(kind, bodies) };
  }
  const designation = viewerDesignation(system);
  const read = toSystemBodiesModel(bodies.response, designation);
  if (read.kind === "fault") {
    return {
      kind: "failed",
      command: {
        kind,
        state: { kind: "rejected", code: "internal", reason: read.fault },
        field: null,
      },
    };
  }
  const place: SystemPlace = { kind: "charted", system, designation, barycentre };
  const { drawn } = planet.response;
  return {
    kind: "opened",
    inspection: {
      universe,
      place,
      model: read.model,
      bodies: read.bodies,
      body,
      time: EPOCH,
    },
    inputs: fromInputsDto(planet.response.inputs),
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
): PlanetViewer {
  const [inputs, setInputsState] = useState<PlanetInputs>(DEFAULT_INPUTS);
  const [command, setCommand] = useState<ViewerCommand | null>(null);
  const [shown, setShown] = useState<ShownPlanet | null>(null);
  const [drawSeed, setDrawSeed] = useState<SeedHex>(FIRST_DRAW_SEED);
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
    return controller.signal;
  }, []);

  /** Opens `inputs`' planet for `kind`, reporting the outcome unless `signal` has aborted. */
  const open = useCallback(
    async (
      kind: ViewerCommandKind,
      asking: UniverseIdHex,
      given: PlanetInputs,
      signal: AbortSignal,
    ): Promise<boolean> => {
      const outcome = await generated(requests, asking, given, kind, signal);
      if (signal.aborted || outcome.kind === "aborted") {
        return false;
      }
      if (outcome.kind === "failed") {
        setCommand(outcome.command);
        return false;
      }
      const option = openedOption(outcome.inspection);
      if (typeof option === "string") {
        setCommand({
          kind,
          state: { kind: "rejected", code: "bad_request", reason: option },
          field: null,
        });
        return false;
      }
      const survey = surveyInspected(requests, outcome.inspection).then((settled) => {
        if (settled.kind === "refused") {
          console.warn(`the viewer's survey of ${settled.body} was refused: ${settled.reason}`);
        }
        return settled;
      });
      setInputsState(outcome.inputs);
      setShown((previous) => ({
        body: outcome.inspection.body,
        place: outcome.inspection.place,
        inputs: outcome.inputs,
        drawn: outcome.drawn,
        opening: { option, sequence: (previous?.opening.sequence ?? 0) + 1, survey },
      }));
      setCommand({ kind, state: { kind: "shown" }, field: null });
      return true;
    },
    [requests],
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
    if (universe === null) {
      return;
    }
    const signal = start();
    setCommand({ kind: "generate", state: { kind: "pending" }, field: null });
    open("generate", universe, inputs, signal).catch((error: unknown) => {
      failed("generate", signal, error);
    });
  }, [universe, inputs, start, open, failed]);

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
      if (await open("randomize", universe, drawnInputs, signal)) {
        setLastDraw(seed);
        setDrawSeed(nextDrawSeed(seed));
      }
    };
    run().catch((error: unknown) => {
      failed("randomize", signal, error);
    });
  }, [universe, drawSeed, requests, start, open, failed]);

  const setInput = useCallback((name: NumericInputName, wire: number | null): void => {
    setInputsState((previous) => ({ ...previous, values: { ...previous.values, [name]: wire } }));
  }, []);
  const setSeed = useCallback((seed: SeedHex): void => {
    setInputsState((previous) => ({ ...previous, seed }));
  }, []);
  const setInputs = useCallback((next: PlanetInputs): void => {
    setInputsState(next);
  }, []);

  return {
    inputs,
    setInput,
    setSeed,
    setInputs,
    command,
    shown,
    generate,
    randomize,
    drawSeed,
    setDrawSeed,
    lastDraw,
  };
}
