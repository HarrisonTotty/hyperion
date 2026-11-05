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
import type { InspectionSurvey } from "../../view/scenes/inspectionSurvey";
import { viewProvenance } from "./serverScene";
import { SCENE_OPTIONS, type SceneOption, SERVER_SCENE_NAME } from "./viewRun";

/**
 * The detail level the view asks of the scene for every body; the server grants each its own
 * (R03, Design note 13).
 */
const SCENE_DETAIL: DetailLevelDto = "full";

/**
 * One opening of `VIEW`'s body inspection, from the `SYSTEM` display's `VIEW BODY` (the owner's
 * request, 2026-11-01): its kept scene, made once as it is opened, how many openings came before,
 * so that each is a new scene, and the survey of its body sent as it was opened.
 */
export interface InspectionOpening {
  readonly option: SceneOption;
  /** Counts the openings from 1, by which `VIEW` makes a new stage for each. */
  readonly sequence: number;
  /** The whole body's survey (`surveyInspected`), which never rejects. */
  readonly survey: Promise<InspectionSurvey>;
}

/**
 * The body inspection's kept scene as the `SCENE` selector offers it, made once: its frames are
 * pure functions of the script's time.
 *
 * @throws Error from `inspectionScene` for a body it cannot draw, which `SYSTEM` never opens.
 */
export function inspectionOption(inspection: BodyInspection): SceneOption {
  const kept = inspectionScene(inspection);
  return { name: kept.name, make: () => kept };
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
  readonly inspection: InspectionOpening | null;
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
   * before the first: each new opening is shown at once.
   */
  readonly inspection: InspectionOpening | null;
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
 * ID. The subscription is open only while the display is shown, as it was when the display held
 * it; the choice outlives a hide. Every push re-renders what it wraps, about once a second; the
 * drawing loop reads positions through `frameAt` with no render.
 *
 * A new opening of the body inspection (the owner's request, 2026-11-01) is shown as it comes, and
 * stays among the `SCENE` choices until it is closed: the scene shown is derived, the inspection
 * while its opening is neither closed nor chosen away from, and else the operator's last other
 * choice, which an opening never changes, so that closing the inspection returns `VIEW` to the
 * scene shown before it. The inspection is never the kept scene that stands in for the server's.
 */
export function ViewSceneProvider({
  active,
  knownSystem,
  inspection,
  children,
}: ViewSceneProviderProps) {
  // The operator's last choice but the inspection.
  const [chosen, setChosen] = useState(SERVER_SCENE_NAME);
  const [keptName, setKeptName] = useState(SCENE_OPTIONS[0]?.name ?? "");
  // The opening the operator chose another scene over, and the one closed, by their counts.
  const [away, setAway] = useState<number | null>(null);
  const [closed, setClosed] = useState<number | null>(null);
  const open = inspection !== null && inspection.sequence !== closed ? inspection : null;
  const openSequence = open?.sequence ?? null;
  const sceneName = open !== null && open.sequence !== away ? INSPECTION_SCENE_NAME : chosen;
  const universe = useUniverse().open?.id ?? null;
  const designate = useCallback(
    (system: SystemIdHex): string =>
      knownSystem?.system === system ? knownSystem.designation : system,
    [knownSystem],
  );
  const scene = useScene(active ? universe : null, { detail: SCENE_DETAIL, designate });
  const choose = useCallback(
    (name: string): void => {
      if (name === INSPECTION_SCENE_NAME) {
        setAway(null);
        return;
      }
      setChosen(name);
      setAway(openSequence);
      if (name !== SERVER_SCENE_NAME) {
        setKeptName(name);
      }
    },
    [openSequence],
  );
  const closeInspection = useCallback((): void => {
    setClosed(openSequence);
  }, [openSequence]);
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
  // What it wraps is made again for its provenance, not at every push of the scene, which reaches
  // the scene's consumers through the context alone (lane perf-render-thread-2).
  const provenance = viewProvenance(sceneName, scene);
  const content = useMemo(() => children(provenance), [children, provenance]);
  return <ViewSceneContext value={host}>{content}</ViewSceneContext>;
}
