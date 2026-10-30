import type { ReactNode } from "react";

import { formatSci, formatSignificant } from "../../lib/format";
import {
  catalogueClassWords,
  featureMarkLabel,
  memberClassWords,
  shellPhaseWords,
} from "../../lib/galaxy/featureWords";
import type {
  ChartFeature,
  ChartSystem,
  RemnantState,
  ShellState,
  SupernovaState,
} from "../../lib/galaxy/model";
import { populationLabel } from "../../lib/galaxy/wire";
import { formatHex64 } from "../../lib/seed";
import { SystemReading as Reading } from "./SystemReading";

/** The words for what is not there: no remnant, no nebula, no emission. */
const NONE = "NONE";

/** What the `ORIGIN` reading shows: a value, a status in its place, or the em dash. */
type OriginShown =
  | { readonly kind: "value"; readonly text: string }
  | { readonly kind: "status"; readonly text: string }
  | { readonly kind: "missing" };

/**
 * A system's `ORIGIN`: its birth population for one its layer's grid placed, its feature's kind and
 * designation for a feature's member (`OC GPF F207`, or its feature's ID where the chart holds no
 * such feature), `CENTRE` for the centre's, its class in words for a catalogue system; Missing for
 * an origin or a class this build does not know; and `NOT YET MODELLED`, a status, for a row from a
 * server that does not say.
 */
function originShown(system: ChartSystem, features: ReadonlyArray<ChartFeature>): OriginShown {
  const { origin } = system;
  let shown: OriginShown;
  switch (origin.kind) {
    case "not_sent":
      shown = { kind: "status", text: "NOT YET MODELLED" };
      break;
    case "grid":
      shown = { kind: "value", text: populationLabel(system.population) };
      break;
    case "feature_member": {
      const feature = features.find((each) => each.id === origin.feature);
      shown = {
        kind: "value",
        text:
          feature === undefined
            ? formatHex64(origin.feature)
            : featureMarkLabel(feature.kind, feature.designation),
      };
      break;
    }
    case "centre_member":
      shown = { kind: "value", text: "CENTRE" };
      break;
    case "catalogue_system": {
      const words = catalogueClassWords(origin.catalogueClass);
      shown = words === null ? { kind: "missing" } : { kind: "value", text: words };
      break;
    }
    case "unknown":
      shown = { kind: "missing" };
      break;
  }
  return shown;
}

/** Whether a system is a member of a feature or of the centre, whose readout names its class. */
function isMember(system: ChartSystem): boolean {
  return (
    system.origin.kind === "feature_member" ||
    system.origin.kind === "centre_member" ||
    system.memberClass !== null
  );
}

/** What a supernova catalogue entry is, in words. */
function supernovaWords(state: SupernovaState): string {
  let words: string;
  switch (state.kind) {
    case "progenitor":
      words = "PROGENITOR";
      break;
    case "supernova":
      words = "REMNANT SHELL";
      break;
    case "bare_remnant":
      words = "BARE REMNANT";
      break;
  }
  return words;
}

/** What a shell shines in, its parts joined by a middle dot, or `NONE`. */
function shellEmissionWords(shell: ShellState): string {
  const parts = [
    shell.xRayAndRadio ? "X-RAY AND RADIO" : null,
    shell.optical ? "OPTICAL" : null,
    shell.neutralHydrogen ? "H I" : null,
  ].filter((part) => part !== null);
  return parts.length === 0 ? NONE : parts.join(" · ");
}

interface RemnantReadingsProps {
  readonly remnant: RemnantState;
  readonly stale: boolean;
}

/** What a supernova left: its compact remnant's offset and bow shock, its nebula, a survivor. */
function RemnantReadings({ remnant, stale }: RemnantReadingsProps) {
  return (
    <>
      <Reading
        label="REMNANT OFFSET"
        value={remnant.offsetLy === null ? NONE : formatSignificant(remnant.offsetLy)}
        unit={remnant.offsetLy === null ? undefined : "ly"}
        stale={stale}
      />
      <Reading label="BOW SHOCK" value={remnant.bowShock ? "YES" : "NO"} stale={stale} />
      <Reading
        label="PWN POWER"
        value={remnant.nebulaPowerW === null ? NONE : formatSci(remnant.nebulaPowerW)}
        unit={remnant.nebulaPowerW === null ? undefined : "W"}
        stale={stale}
      />
      {remnant.survivorOffsetLy === null ? null : (
        <Reading
          label="SURVIVOR OFFSET"
          value={formatSignificant(remnant.survivorOffsetLy)}
          unit="ly"
          stale={stale}
        />
      )}
    </>
  );
}

interface SupernovaReadingsProps {
  readonly state: SupernovaState;
  readonly stale: boolean;
}

/** A supernova catalogue entry's state at the chart's time, by what it is. */
function SupernovaReadings({ state, stale }: SupernovaReadingsProps) {
  const head = <Reading label="SUPERNOVA" value={supernovaWords(state)} wide stale={stale} />;
  let content: ReactNode;
  switch (state.kind) {
    case "progenitor":
      content = (
        <>
          {head}
          <Reading
            label="EXPLODES IN"
            value={formatSignificant(state.untilExplosionYr)}
            unit="yr"
            stale={stale}
          />
          {state.pairPeriodS === null ? null : (
            <Reading
              label="PAIR PERIOD"
              value={formatSignificant(state.pairPeriodS)}
              unit="s"
              stale={stale}
            />
          )}
        </>
      );
      break;
    case "supernova":
      content = (
        <>
          {head}
          <Reading label="SN AGE" value={formatSignificant(state.ageYr)} unit="yr" stale={stale} />
          <Reading
            label="SHELL RADIUS"
            value={formatSignificant(state.shell.radiusLy)}
            unit="ly"
            stale={stale}
          />
          <Reading
            label="SHELL PHASE"
            value={shellPhaseWords(state.shell.phase)}
            wide
            stale={stale}
          />
          <Reading
            label="SHOCK SPEED"
            value={formatSignificant(state.shell.shockSpeedKmS)}
            unit="km/s"
            stale={stale}
          />
          <Reading
            label="SHELL EMISSION"
            value={shellEmissionWords(state.shell)}
            wide
            stale={stale}
          />
          <RemnantReadings remnant={state.remnant} stale={stale} />
        </>
      );
      break;
    case "bare_remnant":
      content = (
        <>
          {head}
          <Reading label="SN AGE" value={formatSignificant(state.ageYr)} unit="yr" stale={stale} />
          <RemnantReadings remnant={state.remnant} stale={stale} />
        </>
      );
      break;
  }
  return content;
}

/** Props of {@link OriginReadings}. */
export interface OriginReadingsProps {
  readonly system: ChartSystem;
  /** The features of the chart on show, whose members' feature is named by its designation. */
  readonly features: ReadonlyArray<ChartFeature>;
  /** Whether the chart's answer, which the row is read from, is stale. */
  readonly stale: boolean;
}

/**
 * The system readout's readings of what placed the selected system (plan 09, P09.T41.c): its
 * `ORIGIN`, a member's `MEMBER CLASS`, and a supernova catalogue entry's state.
 *
 * @remarks
 * Readings of the row, a `dt` and its `dd` each, for the readout's own list. `ORIGIN` always
 * stands, a status where the server does not say, so that the reading never moves; `MEMBER CLASS`
 * stands for a member of a feature or of the centre, Missing where its class is one this build does
 * not know or the row carries none; a supernova entry's readings stand while the row carries its
 * state: `SUPERNOVA` (`PROGENITOR`, `REMNANT SHELL` or `BARE REMNANT`), then the progenitor's time
 * left and a Type Ia pair's period, or the shell's age, radius, phase, shock speed and what it
 * shines in, and what the explosion left: the compact remnant's offset and bow shock, its pulsar's
 * wind nebula's power, and a Type Ia survivor's offset. An origin, a catalogue class or a member
 * class this build does not know reads Missing, and every other reading as before (plan 04's
 * reserved list).
 */
export function OriginReadings({ system, features, stale }: OriginReadingsProps) {
  const origin = originShown(system, features);
  return (
    <>
      <Reading
        label="ORIGIN"
        value={origin.kind === "missing" ? null : origin.text}
        wide
        stale={stale}
        status={origin.kind === "status"}
      />
      {isMember(system) ? (
        <Reading
          label="MEMBER CLASS"
          value={system.memberClass === null ? null : memberClassWords(system.memberClass)}
          wide
          stale={stale}
        />
      ) : null}
      {system.supernova === null ? null : (
        <SupernovaReadings state={system.supernova} stale={stale} />
      )}
    </>
  );
}
