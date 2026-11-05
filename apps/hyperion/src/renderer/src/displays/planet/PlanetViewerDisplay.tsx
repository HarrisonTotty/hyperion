import { memo, useCallback, useEffect, useId, useMemo } from "react";

import { StatusText } from "../../components/StatusText";
import { useServerLink } from "../../lib/serverLink";
import { isTextEntry } from "../../lib/textEntry";
import { useUniverse } from "../../lib/universe";
import type { QualitySetting } from "../../view/quality/qualitySetting";
import type { AtmosphereLeversOverride, GeometryOverride } from "../../../../preload/api";
import { ViewDisplay } from "../view/ViewDisplay";
import { ViewSceneProvider } from "../view/ViewSceneProvider";
import { GENERATE_KEY, PlanetInputsPanel } from "./PlanetInputsPanel";
import { PlanetViewerBar, RANDOMIZE_KEY } from "./PlanetViewerBar";
import { SunControl } from "./SunControl";
import { usePlanetViewer } from "./usePlanetViewer";
import { heldSunStatement } from "./viewerScene";

/** Props of {@link PlanetViewerDisplay}: what `App` hands `VIEW`, which the viewer draws with. */
export interface PlanetViewerDisplayProps {
  readonly setting: QualitySetting;
  readonly platform: string;
  readonly performanceThreads: number | null;
  readonly geometry: GeometryOverride | undefined;
  readonly atmosphereLevers: AtmosphereLeversOverride | undefined;
}

/**
 * The `PLANET VIEWER` display (plan R15): one planet, generated from the generator's own inputs or
 * drawn from the galaxy, drawn by `VIEW` itself from orbit down to its terrain.
 *
 * @remarks
 * - Its `INPUTS` panel stands on the left ({@link PlanetInputsPanel}), and on the right its bar of
 *   `RANDOMIZE` and `SHARE` ({@link PlanetViewerBar}) above `VIEW`'s whole display: its engine, terrain session, atmosphere cache, exposure, styles and instrument
 *   views, held by a `ViewSceneProvider` of its own whose one scene is the planet's body inspection,
 *   with no `SCENE` selector and no subscription to the server's scene. Each planet shown is a new
 *   opening, its camera framed afresh.
 * - Until a planet is shown the view's place says how to show one. While `SUN` is `HELD` the
 *   view's label block says where (`heldSunStatement`), since the light is not the records'.
 * - `G` generates and `N` randomizes from anywhere on the display but a text field and a view's
 *   canvas, whose letters are the flight keys.
 * - Hidden, it costs nothing: `Activity` tears its effects down, `VIEW`'s engine, loop, terrain
 *   session and atmosphere cache with them, and it asks nothing.
 */
function PlanetViewerPanels({
  setting,
  platform,
  performanceThreads,
  geometry,
  atmosphereLevers,
}: PlanetViewerDisplayProps) {
  const heldBackId = useId();
  const link = useServerLink();
  const universe = useUniverse().open?.id ?? null;
  const viewer = usePlanetViewer(link.requests, universe);
  const heldBack =
    universe === null
      ? "no universe open, open one on the GALAXY display"
      : link.status === "connected"
        ? null
        : "no link to the server";
  const { generate, randomize: draw } = viewer;
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey || event.repeat) {
        return;
      }
      if (isTextEntry(event.target) || event.target instanceof HTMLCanvasElement) {
        return;
      }
      const key = event.key.toLowerCase();
      if (key === GENERATE_KEY && heldBack === null) {
        event.preventDefault();
        generate();
      } else if (key === RANDOMIZE_KEY && heldBack === null) {
        event.preventDefault();
        draw();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [generate, draw, heldBack]);
  const sunStatement = heldSunStatement(viewer.sun);
  const hostStatements = useMemo(
    () => (sunStatement === null ? NO_STATEMENTS : [sunStatement]),
    [sunStatement],
  );
  const renderView = useCallback(
    () => (
      <ViewDisplay
        sceneBar={false}
        sceneOptions={NO_KEPT_SCENES}
        hostStatements={hostStatements}
        setting={setting}
        platform={platform}
        performanceThreads={performanceThreads}
        geometry={geometry}
        atmosphereLevers={atmosphereLevers}
      />
    ),
    [setting, platform, performanceThreads, geometry, atmosphereLevers, hostStatements],
  );
  const { shown } = viewer;
  return (
    <div className="planet-viewer">
      <div className="planet-viewer__inputs">
        <PlanetInputsPanel
          viewer={viewer}
          heldBack={heldBack}
          heldBackId={heldBackId}
          sun={<SunControl sun={viewer.sun} onSun={viewer.setSun} />}
        />
      </div>
      <div className="planet-viewer__view">
        <PlanetViewerBar
          inputs={viewer.inputs}
          onLoad={viewer.setInputs}
          drawSeed={viewer.drawSeed}
          onDrawSeedChange={viewer.setDrawSeed}
          lastDraw={viewer.lastDraw}
          command={viewer.command}
          heldBack={heldBack}
          heldBackId={heldBackId}
          onRandomize={draw}
        />
        {shown === null ? (
          <p className="panel__empty planet-viewer__empty">
            <StatusText text="NO PLANET: GENERATE the inputs, or RANDOMIZE a planet of the galaxy" />
          </p>
        ) : (
          <ViewSceneProvider active={false} knownSystem={shown.place} inspection={shown.opening}>
            {renderView}
          </ViewSceneProvider>
        )}
      </div>
    </div>
  );
}

/** The viewer offers `VIEW` no kept test scene: its one scene is its planet's inspection. */
const NO_KEPT_SCENES: ReadonlyArray<never> = [];

/** The label block's statements of a sun where the records put it: none. */
const NO_STATEMENTS: ReadonlyArray<string> = [];

/** The `PLANET VIEWER` display (see {@link PlanetViewerPanels}), memoised as `VIEW` is. */
export const PlanetViewerDisplay = memo(PlanetViewerPanels);
