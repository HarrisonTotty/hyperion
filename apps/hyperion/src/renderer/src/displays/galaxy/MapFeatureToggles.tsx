import type { MapFeatureKindWire } from "@hyperion/protocol";

import { mapFeatureKindLabel } from "../../lib/galaxy/featureWords";
import { MAP_FEATURE_KINDS } from "../../lib/galaxy/featureWire";
import { MAP_FEATURE_MARK_REM } from "../../lib/galaxy/mapFeatures";
import { LegendSymbol } from "../../spatial/LegendSymbol";

/** Props of {@link MapFeatureToggles}. */
export interface MapFeatureTogglesProps {
  /** The kinds the overlay draws. */
  readonly shown: ReadonlySet<MapFeatureKindWire>;
  /** Puts a kind on the overlay or takes it off. */
  readonly onToggle: (kind: MapFeatureKindWire) => void;
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

/**
 * The galaxy map's `FEATURES` overlay, one toggle per kind, each its legend too (plan 09,
 * P09.T41.b): `GC` and `CENTRE` beside the open Greek cross the map draws them with, and `SNR`, a
 * bright shell, beside a circle, since a shell is drawn at its true radius alone.
 *
 * @remarks
 * Checkboxes in a group, as the page's other choices are radio buttons in one, every kind on to
 * start with. A display control, which only changes what the console shows and asks nothing of
 * the server, so it is never held back, not even while the link is down. Where a kind stands, its
 * request pending or failed or the server not generating it, the page says on its line for the
 * views' wait.
 */
export function MapFeatureToggles({ shown, onToggle }: MapFeatureTogglesProps) {
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
      </div>
    </fieldset>
  );
}
