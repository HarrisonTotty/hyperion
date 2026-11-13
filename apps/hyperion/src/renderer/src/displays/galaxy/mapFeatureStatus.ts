import type { MapFeatureKindWire } from "@hyperion/protocol";

import { annunciation } from "../../components/RequestStatus";
import type { StatusStanding } from "../../components/StatusLine";
import { mapFeatureKindLabel, notYetModelledWords } from "../../lib/galaxy/featureWords";
import type { GalaxyFeaturesQuery } from "./useGalaxyFeatures";

/** What the galaxy map's page says of its feature overlay, on its line for the views' wait. */
export interface MapFeatureStatus {
  /** The words: `FEATURES PENDING`, `FEATURES TIMED OUT`, `SNR NOT YET MODELLED`. */
  readonly text: string;
  readonly standing: StatusStanding;
  /** Whether `RETRY` follows, which asks for the features again. */
  readonly retry: boolean;
}

/**
 * What the page says of its feature overlay (plan 09, P09.T41.b), or `null` while there is nothing
 * to say: the request's own state while it has not answered, its subject `FEATURES` before it
 * (`FEATURES PENDING`, the guide's statement that a feature layer still being computed says so,
 * so that unfetched features never look absent; `FEATURES TIMED OUT` and `FEATURES REJECTED:` with
 * `RETRY`); `FEATURE DATA INVALID:` and its cause, with `RETRY`, for an answer that cannot be
 * drawn; and once it has answered, the shown kinds the server does not generate, in the data
 * state for what this generator version does not compute, `SNR NOT YET MODELLED`, so that none is
 * read as none.
 *
 * @remarks
 * The link's state is the link's, which the page states first whatever this says, so a request
 * held by a link that is down says nothing here.
 */
export function mapFeatureStatus(
  query: GalaxyFeaturesQuery,
  shown: ReadonlySet<MapFeatureKindWire>,
): MapFeatureStatus | null {
  const { state, layers, fault } = query;
  if (fault !== null) {
    return { text: `FEATURE DATA INVALID: ${fault}`, standing: "fault", retry: true };
  }
  const said = state.kind === "link_down" ? null : annunciation(state);
  if (said !== null) {
    return {
      text: `FEATURES ${said.text}`,
      standing: said.standing,
      retry: said.standing !== "waiting",
    };
  }
  const missing = (layers ?? []).flatMap((layer) =>
    layer.status === "not_generated" && shown.has(layer.kind)
      ? [mapFeatureKindLabel(layer.kind)]
      : [],
  );
  return missing.length === 0
    ? null
    : { text: notYetModelledWords(missing), standing: "waiting", retry: false };
}
