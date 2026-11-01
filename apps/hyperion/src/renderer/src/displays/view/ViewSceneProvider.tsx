import type { DetailLevelDto, SystemIdHex } from "@hyperion/protocol";
import { createContext, type ReactNode, useCallback, useMemo, useState } from "react";

import type { SystemPlace } from "../../lib/scene/model";
import { type SceneView, useScene } from "../../lib/scene/useScene";
import { useUniverse } from "../../lib/universe";
import type { SceneProvenance } from "../../view/scene/model";
import {
  type BodyInspection,
  INSPECTION_SCENE_NAME,
  inspectionScene,
} from "../../view/scenes/inspectionScene";
import { viewProvenance } from "./serverScene";
import { SCENE_OPTIONS, type SceneOption, SERVER_SCENE_NAME } from "./viewRun";

/**
 * The detail level the view asks of the scene for every body; the server grants each its own
 * (R03, Design note 13).
 */
const SCENE_DETAIL: DetailLevelDto = "full";

/**
 * One opening of `VIEW`'s body inspection, from the `SYSTEM` display's `VIEW BODY` (the owner's
 * request, 2026-11-01): the body with its system's records, and how many openings came before, so
 * that each is a new scene.
 */
export interface InspectionOpening {
  readonly inspection: BodyInspection;
  /** Counts the openings from 1; each opening is a new scene. */
  readonly sequence: number;
}

/** The body inspection `VIEW` offers while it is open: its kept scene and its opening's count. */
export interface OpenInspection {
  readonly option: SceneOption;
  /** Its opening's count, by which `VIEW` makes a new stage for each opening. */
  readonly sequence: number;
}

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
  /** The body inspection, offered under `SCENE` from its opening until it is closed; else `null`. */
  readonly inspection: OpenInspection | null;
  /** Closes the inspection, returning `VIEW` to the scene chosen before it where it is shown. */
  readonly closeInspection: () => void;
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
  /**
   * The latest opening of the body inspection (`App`'s, from `SYSTEM`'s `VIEW BODY`), or `null`
   * before the first: each new opening is chosen at once.
   */
  readonly inspection: InspectionOpening | null;
  /** What it wraps, given where the scene `VIEW` draws comes from, kept or the server's. */
  readonly children: (provenance: SceneProvenance["kind"]) => ReactNode;
}

/** Where the body inspection stands: its opening's count, the scene before it, whether closed. */
interface InspectionState {
  readonly sequence: number;
  /** The scene chosen when it opened, to which closing it returns. */
  readonly before: string;
  readonly closed: boolean;
}

/**
 * Holds the `VIEW` display's scene above the console frame (plan R02, R02.T17): the subscription
 * to the open universe's scene (`useScene`) and the `SCENE` choice, so that the header strip's
 * `TRAINING` banner is computed during render from what `VIEW` draws (delegated decision 3).
 *
 * @remarks
 * The scene states its system's designation (R03.T16); for a server that does not, `designate`
 * names the system the client was last told of by its designation of record and any other by its
 * ID. The subscription is open only while the display is shown, as it was when the display held
 * it; the choice outlives a hide. Every push re-renders what it wraps, about once a second; the
 * drawing loop reads positions through `frameAt` with no render.
 *
 * A new opening of the body inspection (the owner's request, 2026-11-01) is chosen as it comes,
 * the scene chosen before it kept, and the inspection stays among the `SCENE` choices until it is
 * closed, which returns `VIEW` to that scene where the inspection is shown; an opening while one is
 * shown takes its place and keeps the scene before. The inspection is never the kept scene that
 * stands in for the server's.
 */
export function ViewSceneProvider({
  active,
  knownSystem,
  inspection,
  children,
}: ViewSceneProviderProps) {
  const [sceneName, setSceneName] = useState(SERVER_SCENE_NAME);
  const [keptName, setKeptName] = useState(SCENE_OPTIONS[0]?.name ?? "");
  const [inspected, setInspected] = useState<InspectionState | null>(null);
  if (inspection !== null && inspection.sequence !== inspected?.sequence) {
    const shown = inspected !== null && !inspected.closed && sceneName === INSPECTION_SCENE_NAME;
    setInspected({
      sequence: inspection.sequence,
      before: shown ? inspected.before : sceneName,
      closed: false,
    });
    setSceneName(INSPECTION_SCENE_NAME);
  }
  const opened =
    inspection !== null && inspected?.sequence === inspection.sequence && !inspected.closed
      ? inspection
      : null;
  // The scene is made once an opening: its frames are pure functions of the script's time.
  const open = useMemo((): OpenInspection | null => {
    if (opened === null) {
      return null;
    }
    const kept = inspectionScene(opened.inspection);
    return { option: { name: kept.name, make: () => kept }, sequence: opened.sequence };
  }, [opened]);
  const universe = useUniverse().open?.id ?? null;
  const designate = useCallback(
    (system: SystemIdHex): string =>
      knownSystem?.system === system ? knownSystem.designation : system,
    [knownSystem],
  );
  const scene = useScene(active ? universe : null, { detail: SCENE_DETAIL, designate });
  const choose = useCallback((name: string): void => {
    setSceneName(name);
    if (name !== SERVER_SCENE_NAME && name !== INSPECTION_SCENE_NAME) {
      setKeptName(name);
    }
  }, []);
  const closeInspection = useCallback((): void => {
    if (inspected === null || inspected.closed) {
      return;
    }
    setInspected({ ...inspected, closed: true });
    if (sceneName === INSPECTION_SCENE_NAME) {
      setSceneName(inspected.before);
    }
  }, [inspected, sceneName]);
  const host = useMemo(
    (): ViewSceneHost => ({
      scene,
      sceneName,
      keptName,
      knownSystem,
      choose,
      inspection: open,
      closeInspection,
    }),
    [scene, sceneName, keptName, knownSystem, choose, open, closeInspection],
  );
  return (
    <ViewSceneContext value={host}>{children(viewProvenance(sceneName, scene))}</ViewSceneContext>
  );
}
