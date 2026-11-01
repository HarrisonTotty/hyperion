import type { MapFeatureKindWire } from "@hyperion/protocol";
import type { ReactNode } from "react";

import { mapFeatureKindLabel } from "../../lib/galaxy/featureWords";
import { MAP_FEATURE_KINDS } from "../../lib/galaxy/featureWire";
import { MAP_FEATURE_MARK_REM } from "../../lib/galaxy/mapFeatures";
import {
  DWARF_CORE_LABEL,
  STREAM_LABEL,
  STREAM_OVERLAY_KINDS,
  type StreamOverlayKind,
} from "../../lib/galaxy/streamTracks";
import { LegendSymbol } from "../../spatial/LegendSymbol";

/** Each stream overlay kind's name, the guide's term. */
const STREAM_KIND_LABEL: Readonly<Record<StreamOverlayKind, string>> = {
  stream: STREAM_LABEL,
  dwarf_core: DWARF_CORE_LABEL,
};

/** Props of {@link MapFeatureToggles}. */
export interface MapFeatureTogglesProps {
  /** The kinds the feature overlay draws. */
  readonly shown: ReadonlySet<MapFeatureKindWire>;
  /** Puts a kind on the feature overlay or takes it off. */
  readonly onToggle: (kind: MapFeatureKindWire) => void;
  /** The kinds the stream overlay draws. */
  readonly streamKinds: ReadonlySet<StreamOverlayKind>;
  /** Puts a kind on the stream overlay or takes it off. */
  readonly onToggleStream: (kind: StreamOverlayKind) => void;
}

/** A bright shell's sample: a circle drawn as its line is, open, 1 px at the line scale. */
function ShellSample() {
  return (
    <svg
      className="symbol-legend__mark map-features__sample"
      style={{
        width: `${String(MAP_FEATURE_MARK_REM)}rem`,
        height: `${String(MAP_FEATURE_MARK_REM)}rem`,
      }}
      viewBox="-5 -5 10 10"
      aria-hidden="true"
      focusable="false"
    >
      <circle r="4.5" />
    </svg>
  );
}

/** A stream's sample: a short run of its track's line, as the map draws it. */
function StreamSample() {
  return (
    <svg
      className="symbol-legend__mark map-features__sample"
      style={{
        width: `${String(MAP_FEATURE_MARK_REM)}rem`,
        height: `${String(MAP_FEATURE_MARK_REM)}rem`,
      }}
      viewBox="-5 -5 10 10"
      aria-hidden="true"
      focusable="false"
    >
      <line x1="-5" y1="2" x2="5" y2="-2" />
    </svg>
  );
}

/** Each stream overlay kind's sample, as the map draws it: a run of its line, or the open cross. */
const STREAM_KIND_SAMPLE: Readonly<Record<StreamOverlayKind, ReactNode>> = {
  stream: <StreamSample />,
  dwarf_core: (
    <LegendSymbol shape="greek-cross" diameterRem={MAP_FEATURE_MARK_REM} filled={false} />
  ),
};

/**
 * The galaxy map's `FEATURES` overlay, one toggle per kind, each its legend too (plan 09,
 * P09.T41.b): `GC` and `CENTRE` beside the open Greek cross the map draws them with, and `SNR`, a
 * bright shell, beside a circle, since a shell is drawn at its true radius alone; then the stream
 * overlay's (plan 10, P10.T11), `STREAM` beside a run of its track's line and `DWARF CORE` beside
 * the open cross, a dwarf core being a feature.
 *
 * @remarks
 * Checkboxes in a group, as the page's other choices are radio buttons in one, every kind on to
 * start with. A display control, which only changes what the console shows and asks nothing of
 * the server, so it is never held back, not even while the link is down. Where a kind stands, its
 * request pending or failed or the server not generating it, the page says on its line for the
 * views' wait, and of `STREAM` and `DWARF CORE` under these toggles.
 */
export function MapFeatureToggles({
  shown,
  onToggle,
  streamKinds,
  onToggleStream,
}: MapFeatureTogglesProps) {
  return (
    <fieldset className="form-choice map-view__features">
      <legend className="form-field__label">FEATURES</legend>
      <div className="form-choice__options">
        {MAP_FEATURE_KINDS.map((kind) => (
          <label key={kind} className="form-choice__option">
            <input
              type="checkbox"
              checked={shown.has(kind)}
              onChange={() => {
                onToggle(kind);
              }}
            />
            {kind === "bright_shell" ? (
              <ShellSample />
            ) : (
              <LegendSymbol shape="greek-cross" diameterRem={MAP_FEATURE_MARK_REM} filled={false} />
            )}
            {mapFeatureKindLabel(kind)}
          </label>
        ))}
        {STREAM_OVERLAY_KINDS.map((kind) => (
          <label key={kind} className="form-choice__option">
            <input
              type="checkbox"
              checked={streamKinds.has(kind)}
              onChange={() => {
                onToggleStream(kind);
              }}
            />
            {STREAM_KIND_SAMPLE[kind]}
            {STREAM_KIND_LABEL[kind]}
          </label>
        ))}
      </div>
    </fieldset>
  );
}
