/**
 * The hand-over check's served world, read from a running server (plan R10, R10.T15.b): the body a
 * `--views-check --terrain --handover --universe <ID> --body <ID>` launch names, whose looks the
 * check then flies in `TERRAIN TEST`'s place (`servedHandoverWorld`).
 *
 * @remarks
 * Read once, before the check's scenes are made:
 *
 * 1. **The body** from the universe's scene (`subscribe` at `full`, unsubscribed once read), as the
 *    server's scene draws it through `fromServer.ts` (`servedBodyOf`).
 * 2. **Its host discs** from the server's sky at the body (`servedHostDiscs`).
 * 3. **Its field** (`surface_field`, whole): its header (relief, level) and its coverage (the
 *    surveyed cells and their middle), the chunks dropped, since each look's session asks for the
 *    field itself. Behind the serve gate the server answers `not_modelled`, which the read gives
 *    as such, for the launch's end to state.
 *
 * Each outcome is the read's result ({@link ServedHandoverRead}), the server's refusals among them,
 * not an exception; an abort of its signal cancels the request in flight. `just place-ship --survey`
 * surveys the body first: a field with no surveyed cell has no looks to judge.
 */
import type { BodyIdHex, RecordSectionDto, RequestClient, UniverseIdHex } from "@hyperion/protocol";

import { toSceneModel } from "../../../lib/scene/sceneWire";
import {
  cancelledOn,
  type HostDiscRequests,
  servedBodyOf,
  servedHostDiscs,
} from "../../../view/scene/servedBody";
import { CoverageView } from "../../../view/terrain/coverage";
import type { TerrainRequests } from "../../../view/terrain/session";
import type { SurfaceEvents } from "../useTerrainSession";
import { type HandoverWorld, servedHandoverWorld } from "./handoverScenes";

/** What the reading asks through: a request client's scene subscription, bulk and plain requests. */
export type ServedHandoverRequests = TerrainRequests &
  HostDiscRequests & { readonly subscribe: RequestClient["subscribe"] };

/**
 * What the reading gave: the world; the body's field not modelled, naming the section the server
 * needs, as behind the serve gate; or why it failed, in words, a step's refusal or a cancel.
 */
export type ServedHandoverRead =
  | { readonly kind: "read"; readonly world: HandoverWorld }
  | { readonly kind: "not served"; readonly section: RecordSectionDto }
  | { readonly kind: "failed"; readonly message: string };

/** A reading that failed, saying why. */
function failed(message: string): ServedHandoverRead {
  return { kind: "failed", message };
}

/** The surface events of a world read whole before its looks: none, its survey held whole. */
const NO_SURFACE_EVENTS: SurfaceEvents = () => () => undefined;

/**
 * The hand-over world of `body` in `universe` as the server serves it ({@link servedHandoverWorld}),
 * each look's session asking through `requests`, or why there is none; `signal`'s abort cancels the
 * request in flight.
 */
export async function readServedHandoverWorld(
  requests: ServedHandoverRequests,
  universe: UniverseIdHex,
  body: BodyIdHex,
  signal: AbortSignal,
): Promise<ServedHandoverRead> {
  const opened = await cancelledOn(
    signal,
    requests.subscribe(universe, { topic: "scene", detail: "full", cameras: [] }),
  );
  if (!opened.ok) {
    return failed(`the scene of ${universe} was refused: ${opened.error.message}`);
  }
  const { topic: _topic, ...state } = opened.subscription.state;
  opened.subscription.unsubscribe();
  // A system the scene states no place for is named by its ID, as `VIEW` names it with none opened.
  const scene = toSceneModel(state, (system) => system);
  if (scene.kind !== "ok") {
    return failed(`the scene of ${universe} is unreadable: ${scene.fault}`);
  }
  const read = servedBodyOf(scene.model, universe, body);
  if (read.kind !== "served") {
    return failed(`${body} is no served world in the scene: ${read.gap}`);
  }
  const { served } = read;
  const discs = await servedHostDiscs(requests, served, signal);
  if (!discs.ok) {
    return failed(discs.message);
  }
  const field = await cancelledOn(
    signal,
    requests.requestBulk<"surface_field">(
      { kind: "surface_field", universe, body, have_revision: null },
      (response) => (response.state === "ready" ? response.bulk : null),
    ),
  );
  if (!field.ok) {
    return failed(`the field of ${body} was refused: ${field.error.message}`);
  }
  const { response } = field;
  if (response.state === "not_modelled") {
    return { kind: "not served", section: response.section };
  }
  const merged = CoverageView.empty(response.header.level).merged(
    response.coverage,
    response.revision,
  );
  if (!merged.ok) {
    return failed(`the coverage of ${body} is unusable: ${merged.message}`);
  }
  const world = servedHandoverWorld({
    served,
    hostDiscs: discs.discs,
    header: response.header,
    coverage: merged.coverage,
    link: { requests, onSurface: NO_SURFACE_EVENTS },
  });
  return world.surveyedCells === 0
    ? failed(`${body} holds no surveyed cell: survey it first (just place-ship --survey)`)
    : { kind: "read", world };
}
