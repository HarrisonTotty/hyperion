import type { BodyIdHex, SystemIdHex, UniverseIdHex } from "@hyperion/protocol";
import { useEffect, useMemo, useRef, useState } from "react";

import { StatusLine, type StatusStanding } from "../../components/StatusLine";
import type { SurfaceEvents } from "../../displays/view/useTerrainSession";
import { type ConnectionStatus, useServerConnection } from "../../lib/connection";
import { isRefusal } from "../../lib/refusal";
import type { SceneModel } from "../../lib/scene/model";
import { type SceneStatus, type SceneSurfaceEvent, useScene } from "../../lib/scene/useScene";
import {
  LINK_STATUS_LABEL,
  ServerLinkContext,
  useServerLink,
  useServerLinkValue,
} from "../../lib/serverLink";
import {
  type HostDiscsRead,
  type ServedBodyRead,
  servedBodyOf,
  servedHostDiscs,
} from "../scene/servedBody";
import { DescentSpike, type DescentSpikeProps } from "./DescentSpike";
import { SPIKE_RETRY } from "./spikeFaults";
import { SpikeHeader } from "./SpikeHeader";
import { type ServedSpikeWorld, servedSpikeWorld } from "./spikeWorld";

/** The status while the served world is read from the server (plan R10, R10.T15.b). */
export const READING_SERVED = "MEASURING TERRAIN: reading the served world";

/** The status of a server of another protocol version, with its remedy (the guide's row). */
export const SERVED_LINK_INCOMPATIBLE = `${LINK_STATUS_LABEL.incompatible}: server of another protocol version, relaunch with a matching server`;

/**
 * The status where the served world's host discs could not be had of the server's sky, a request
 * that a relaunch may answer, with the server's reason in its own words (`reason`, as
 * `servedHostDiscs` gives it), as the guide's row has it: `(<the server's reason>)`.
 */
export function starDiscsNotReceived(reason: string): string {
  return `TERRAIN NOT MEASURED: star discs not received (${reason}), ${SPIKE_RETRY}`;
}

/** The scene's detail asked for every body, as `VIEW` asks it: the server grants each its own. */
const SCENE_DETAIL = "full";

/** Names a system the scene states no place for by its ID, as `VIEW` does with none opened. */
function designateById(system: SystemIdHex): string {
  return system;
}

/**
 * The scene's surface events, relayed to the terrain session: a source stable for the spike's life,
 * whose listeners the scene's subscription feeds.
 */
export class SurfaceRelay {
  readonly #listeners = new Set<(event: SceneSurfaceEvent) => void>();

  /** Listens for the relayed events; returns the unsubscribe. */
  readonly onSurface: SurfaceEvents = (listener) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  /** Hands `event` to every listener. */
  emit(event: SceneSurfaceEvent): void {
    for (const listener of this.#listeners) {
      listener(event);
    }
  }
}

/** A status line's words and how it stands, and whether it ends the run. */
export interface ServedStatus {
  readonly text: string;
  readonly standing: StatusStanding;
  readonly ends: boolean;
}

/**
 * Where the served world's reading stands, in the guide's words (its `MEASURING TERRAIN`,
 * `TERRAIN NOT MEASURED` row), before the world flies: the link's state while it is not up before
 * the scene's first answer (`MEASURING TERRAIN: ESTABLISHING LINK` or `MEASURING TERRAIN: NO
 * CARRIER`, steady in plain text while it waits; `LINK INCOMPATIBLE` with its remedy, which ends the
 * run), the scene's refusal with its reason, in plain text for a refusal of what was asked and as a
 * fault for the server's own failure (`isRefusal`, as `VIEW`'s `serverScene.ts` sets it), or its
 * silence (`SCENE TIMED OUT`), a body that is no served world there with its gap, which another
 * body would mend, the sky's discs not had with the failure's code and words, in plain text, else
 * the reading's own line.
 */
export function servedStatus(
  link: ConnectionStatus,
  scene: SceneStatus,
  read: ServedBodyRead | null,
  discs: HostDiscsRead | null,
): ServedStatus {
  if (link === "incompatible") {
    return { text: SERVED_LINK_INCOMPATIBLE, standing: "fault", ends: true };
  }
  if (scene.kind === "rejected") {
    return {
      text: `SCENE REJECTED: ${scene.reason}, ${SPIKE_RETRY}`,
      standing: isRefusal(scene.code) ? "refused" : "fault",
      ends: true,
    };
  }
  if (scene.kind === "timed_out") {
    return { text: `SCENE TIMED OUT: ${SPIKE_RETRY}`, standing: "fault", ends: true };
  }
  if (read?.kind === "gap") {
    return {
      text: `TERRAIN NOT MEASURED: ${read.body} is no served world (${read.gap}), relaunch with another body`,
      standing: "refused",
      ends: true,
    };
  }
  if (discs?.ok === false) {
    // In plain text: the reason is the request's, a refusal or a lost link alike (the UX lean).
    return { text: starDiscsNotReceived(discs.reason), standing: "refused", ends: true };
  }
  if (read === null && link !== "connected") {
    // Steady and plain: the spike waits for the link, as `VIEW` reads `NO CARRIER` as waiting.
    return {
      text: `MEASURING TERRAIN: ${LINK_STATUS_LABEL[link]}`,
      standing: "waiting",
      ends: false,
    };
  }
  return { text: READING_SERVED, standing: "waiting", ends: false };
}

/** Props of {@link ServedSpikeFlight}. */
interface ServedSpikeFlightProps {
  readonly universe: UniverseIdHex;
  readonly body: BodyIdHex;
  readonly spike: Omit<DescentSpikeProps, "world">;
}

/**
 * Reads the served world from the universe's scene, its first push the body's (`useScene`), and its
 * sky's discs, then flies `DescentSpike` over it, the scene's surface events relayed to its session
 * for the rest of the run; until then, and where it cannot be read, the spike's frame with the
 * reading's status line ({@link servedStatus}).
 */
function ServedSpikeFlight({ universe, body, spike }: ServedSpikeFlightProps) {
  const link = useServerLink();
  const scene = useScene(universe, { detail: SCENE_DETAIL, designate: designateById });
  const [relay] = useState(() => new SurfaceRelay());
  const { onSurface } = scene;
  useEffect(() => onSurface((event) => relay.emit(event)), [onSurface, relay]);
  // The scene as its first push held it, kept for the run: the body is read once, at that push.
  const [first, setFirst] = useState<SceneModel | null>(null);
  if (first === null && scene.model !== null) {
    setFirst(scene.model);
  }
  const read = useMemo(
    () => (first === null ? null : servedBodyOf(first, universe, body)),
    [first, universe, body],
  );
  const served = read?.kind === "served" ? read.served : null;
  const { requests } = link;
  const [discs, setDiscs] = useState<HostDiscsRead | null>(null);
  useEffect(() => {
    if (served === null) {
      return undefined;
    }
    const abort = new AbortController();
    servedHostDiscs(requests, served, abort.signal)
      .then((got) => {
        if (!abort.signal.aborted) {
          setDiscs(got);
        }
        return undefined;
      })
      .catch((error: unknown) => {
        console.error("the descent spike's served world's host discs could not be asked:", error);
      });
    return () => {
      abort.abort();
    };
  }, [served, requests]);
  // The world once read, one object for the run, so that the spike's measurement starts once; the
  // link's state and source follow the link without remaking it.
  const flown = useMemo(
    () => (served === null || discs?.ok !== true ? null : servedSpikeWorld(served, discs.discs)),
    [served, discs],
  );
  const linkUp = link.status === "connected";
  const world = useMemo(
    (): ServedSpikeWorld | null =>
      flown === null
        ? null
        : {
            kind: "served",
            world: flown,
            source: { kind: "server", universe, requests, linkUp, onSurface: relay.onSurface },
            link: link.status,
          },
    [flown, universe, requests, linkUp, relay, link.status],
  );
  const status = world === null ? servedStatus(link.status, scene.status, read, discs) : null;
  const ended = status?.ends === true ? status.text : null;
  // The harness is told once, at the first cause that ends the run: the main process ends it.
  const toldRef = useRef(false);
  const { listeners } = spike;
  useEffect(() => {
    if (ended === null || toldRef.current) {
      return;
    }
    toldRef.current = true;
    console.error(`the descent spike's served world could not be read: ${ended}`);
    listeners?.onFailed?.(ended);
  }, [ended, listeners]);
  if (world !== null) {
    return <DescentSpike {...spike} world={world} />;
  }
  return (
    <div className="spike">
      <SpikeHeader link={link.status} />
      <main className="view spike__work">
        <div className="view__main">
          {status === null ? null : <StatusLine text={status.text} standing={status.standing} />}
        </div>
      </main>
    </div>
  );
}

/** Props of {@link ServedDescentSpike}: the spike's own, and the served world's. */
export interface ServedDescentSpikeProps extends Omit<DescentSpikeProps, "world"> {
  /** The server's WebSocket URL, `window.hyperion.serverUrl`. */
  readonly serverUrl: string;
  /** The served world's universe and body (`--universe`, `--body`). */
  readonly universe: UniverseIdHex;
  readonly body: BodyIdHex;
}

/**
 * The descent spike over a served world (plan R10, R10.T15.b): it links to the server, reads the
 * body from the universe's scene through `fromServer.ts` (`servedBodyOf`) and the host discs that
 * light it from the server's sky (`servedHostDiscs`), then flies `DescentSpike` over it, its terrain
 * session asking the server for the body's field over the link.
 *
 * @remarks
 * `just place-ship --survey` has placed the ship and surveyed the body first (`descentSpike.sh`),
 * so the field holds its survey at its first answer. While the world is read the spike's frame
 * stands, its header with the `MEASUREMENT` banner and the link's annunciator, and its line reads
 * the link's state while the link is not up, then {@link READING_SERVED}; a cause that ends the run
 * reads in the guide's words ({@link servedStatus}) and is told to the listeners' `onFailed` once.
 */
export function ServedDescentSpike({
  serverUrl,
  universe,
  body,
  ...spike
}: ServedDescentSpikeProps) {
  const connection = useServerConnection(serverUrl, __APP_VERSION__);
  const link = useServerLinkValue(connection);
  return (
    <ServerLinkContext value={link}>
      <ServedSpikeFlight universe={universe} body={body} spike={spike} />
    </ServerLinkContext>
  );
}
