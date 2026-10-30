import type { GalaxyFeatures, UniverseIdHex } from "@hyperion/protocol";
import { useState } from "react";

import { toGalaxyFeaturesRequest, toMapFeatureLayers } from "../../lib/galaxy/featureWire";
import type { MapFeatureLayer } from "../../lib/galaxy/model";
import type { Vec3 } from "../../geometry/vec3";
import {
  REQUEST_TIMEOUT_MS,
  type RequestState,
  useServerRequest,
} from "../../lib/useServerRequest";

/** Where the galaxy map's features request stands, and the layers it answered. */
export interface GalaxyFeaturesQuery {
  readonly state: RequestState<"galaxy_features">;
  /**
   * The latest answer's layers for the open universe, kept while a newer request is pending or
   * has failed, or `null` until the first answer and when it cannot be drawn.
   */
  readonly layers: ReadonlyArray<MapFeatureLayer> | null;
  /** Why the answer on hand cannot be drawn, in words, or `null` when nothing is wrong. */
  readonly fault: string | null;
}

/** The answer on hand, with what it was built from, so that it is built once per response. */
interface Kept {
  readonly universe: UniverseIdHex;
  readonly response: GalaxyFeatures;
  readonly layers: ReadonlyArray<MapFeatureLayer>;
}

/** Whether a point of the galaxy can be drawn: every coordinate finite. */
function finite({ x, y, z }: Vec3): boolean {
  return Number.isFinite(x) && Number.isFinite(y) && Number.isFinite(z);
}

/** Whether a radius can be drawn: finite, and not below zero. */
function usableRadius(radiusLy: number): boolean {
  return Number.isFinite(radiusLy) && radiusLy >= 0;
}

/**
 * Why the map's features answer cannot be drawn, in words, or `null` when it can: a feature or a
 * shell with a place or a radius the overlay could not draw, as `MAP DATA INVALID` reads a map's.
 */
export function mapFeatureFault(layers: ReadonlyArray<MapFeatureLayer>): string | null {
  for (const layer of layers) {
    if (layer.status !== "complete") {
      continue;
    }
    const usable =
      layer.kind === "bright_shell"
        ? layer.shells.every((shell) => finite(shell.siteLy) && usableRadius(shell.radiusLy))
        : layer.features.every(
            (feature) => finite(feature.positionLy) && usableRadius(feature.extentLy),
          );
    if (!usable) {
      return "feature unusable";
    }
  }
  return null;
}

/**
 * Asks the server for the features the galaxy map draws (`galaxy_features`, plan 09, P09.T41.b):
 * the globulars, the centre and the bright shells, at the epoch, the time of the map's pictures.
 *
 * @remarks
 * Asked once per open universe, by the page, since both views draw the same features; the server
 * computes them once per universe in the background and answers the request when they are ready,
 * so the overlay's `PENDING` is this request's own state. The answer stays while a newer request
 * is pending or has failed, and goes when another universe is opened. Changing `generation` asks
 * again, which is what `RETRY` does.
 */
export function useGalaxyFeatures(
  universe: UniverseIdHex | null,
  generation: number,
): GalaxyFeaturesQuery {
  const body = universe === null ? null : toGalaxyFeaturesRequest(universe);
  const state = useServerRequest<"galaxy_features">(body, REQUEST_TIMEOUT_MS, generation);
  const [kept, setKept] = useState<Kept | null>(null);

  // Adjusted during render, as the chart's answers are, so that the overlay is drawn in the render
  // its answer arrives in.
  let current = kept;
  if (state.kind === "ok" && universe !== null && kept?.response !== state.response) {
    current = { universe, response: state.response, layers: toMapFeatureLayers(state.response) };
    setKept(current);
  } else if (current !== null && current.universe !== universe) {
    current = null;
    setKept(null);
  }
  const fault = current === null ? null : mapFeatureFault(current.layers);
  return { state, layers: fault === null ? (current?.layers ?? null) : null, fault };
}
