import type { DetailLevelDto, SystemIdHex } from "@hyperion/protocol";
import { createContext, type ReactNode, useCallback, useMemo, useState } from "react";

import type { SystemPlace } from "../../lib/scene/model";
import { type SceneView, useScene } from "../../lib/scene/useScene";
import { useUniverse } from "../../lib/universe";
import type { SceneProvenance } from "../../view/scene/model";
import { viewProvenance } from "./serverScene";
import { SCENE_OPTIONS, SERVER_SCENE_NAME } from "./viewRun";

/**
 * The detail level the view asks of the scene for every body; the server grants each its own
 * (R03, Design note 13).
 */
const SCENE_DETAIL: DetailLevelDto = "full";

/** What the `VIEW` display's scene host holds: the subscription and the `SCENE` choice. */
export interface ViewSceneHost {
  /** The server's scene of the open universe, subscribed while the display is shown. */
  readonly scene: SceneView;
  /** The `SCENE` selector's choice: {@link SERVER_SCENE_NAME} or a kept scene's name. */
  readonly sceneName: string;
  /** The kept scene last chosen, which stands in while the server's cannot be drawn. */
  readonly keptName: string;
  /**
   * The system the client was last told of: the fallback for a server whose scene does not state
   * its system's place (R03.T16).
   */
  readonly knownSystem: SystemPlace | null;
  /** Chooses a scene by its name, as the selector's buttons do. */
  readonly choose: (name: string) => void;
}

/** The `VIEW` display's scene host, which {@link ViewSceneProvider} provides. */
export const ViewSceneContext = createContext<ViewSceneHost | null>(null);

/** Props of {@link ViewSceneProvider}. */
export interface ViewSceneProviderProps {
  /** Whether the `VIEW` display is shown: the scene is subscribed only then. */
  readonly active: boolean;
  /**
   * The system last opened on `SYSTEM`, with its designation of record and barycentre, or `null`.
   */
  readonly knownSystem: SystemPlace | null;
  /** What it wraps, given where the scene `VIEW` draws comes from, kept or the server's. */
  readonly children: (provenance: SceneProvenance["kind"]) => ReactNode;
}

/**
 * Holds the `VIEW` display's scene above the console frame (plan R02, R02.T17): the subscription
 * to the open universe's scene (`useScene`) and the `SCENE` choice, so that the header strip's
 * `TRAINING` banner is computed during render from what `VIEW` draws (delegated decision 3).
 *
 * @remarks
 * The scene states its system's designation (R03.T16); for a server that does not, `designate`
 * names the system the client was last told of by its designation of record and any other by its
 * ID. The subscription is open only
 * while the display is shown, as it was when the display held it; the choice outlives a hide. Every
 * push re-renders what it wraps, about once a second; the drawing loop reads positions through
 * `frameAt` with no render.
 */
export function ViewSceneProvider({ active, knownSystem, children }: ViewSceneProviderProps) {
  const [sceneName, setSceneName] = useState(SERVER_SCENE_NAME);
  const [keptName, setKeptName] = useState(SCENE_OPTIONS[0]?.name ?? "");
  const universe = useUniverse().open?.id ?? null;
  const designate = useCallback(
    (system: SystemIdHex): string =>
      knownSystem?.system === system ? knownSystem.designation : system,
    [knownSystem],
  );
  const scene = useScene(active ? universe : null, { detail: SCENE_DETAIL, designate });
  const choose = useCallback((name: string): void => {
    setSceneName(name);
    if (name !== SERVER_SCENE_NAME) {
      setKeptName(name);
    }
  }, []);
  const host = useMemo(
    (): ViewSceneHost => ({ scene, sceneName, keptName, knownSystem, choose }),
    [scene, sceneName, keptName, knownSystem, choose],
  );
  return (
    <ViewSceneContext value={host}>{children(viewProvenance(sceneName, scene))}</ViewSceneContext>
  );
}
