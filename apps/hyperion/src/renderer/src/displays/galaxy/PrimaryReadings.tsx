import type { UniverseTime } from "@hyperion/protocol";
import type { ReactNode } from "react";

import { SolarUnit } from "../../components/SolarUnit";
import {
  formatAge,
  formatLuminosityLsun,
  formatNumber,
  formatPeriod,
  formatRadiusKm,
  formatRadiusRsun,
  formatSigned,
  formatSignificant,
  formatTemperatureK,
  KM_PER_RSUN,
} from "../../lib/format";
import type { HostBody, HostRemnant, Modelled, Pending, StarEvent } from "../../lib/system/model";
import {
  objectKindLabel,
  phaseLabel,
  remnantLabel,
  starEventLabel,
  variableKindLabel,
} from "../../lib/system/words";
import { SystemReading } from "./SystemReading";

/** Why a light-less object has no luminosity, temperature or magnitude, in the readout's words. */
const NO_LIGHT = "NO LIGHT";

/** Seconds in a day of 86,400 s, the day of every `d`. */
const SECONDS_PER_DAY = 86_400;

/** Nanoseconds in a second. */
const NANOS_PER_SECOND = 1e9;

/** Decimals an absolute magnitude is read to. */
const MAGNITUDE_DECIMALS = 2;

/** What a reading shows: its digits with their unit, or the em dash with why. */
type Shown =
  | { readonly kind: "value"; readonly value: string; readonly unit?: ReactNode }
  | { readonly kind: "missing"; readonly why?: string };

const MISSING: Shown = { kind: "missing" };

/** An object with no light's luminosity, temperature: the em dash, and why. */
const NO_LIGHT_SHOWN: Shown = { kind: "missing", why: NO_LIGHT };

/** A value, with its unit where it has one. */
function shown(value: string, unit?: ReactNode): Shown {
  return unit === undefined ? { kind: "value", value } : { kind: "value", value, unit };
}

/** A value this generator version may not model: the em dash, `NONE`, or the value. */
function modelled<T>(reading: Modelled<T>, show: (present: T) => Shown): Shown {
  let result: Shown;
  switch (reading.kind) {
    case "not_modelled":
      result = MISSING;
      break;
    case "none":
      result = shown("NONE");
      break;
    case "value":
      result = show(reading.value);
      break;
  }
  return result;
}

/** A value the wire leaves out until its task lands: the em dash, or the value. */
function pending<T>(reading: Pending<T>, show: (present: T) => Shown): Shown {
  return reading.kind === "not_modelled" ? MISSING : show(reading.value);
}

interface ReadingProps {
  readonly label: string;
  readonly reading: Shown;
  readonly stale: boolean;
  readonly wide?: boolean;
}

function Reading({ label, reading, stale, wide = false }: ReadingProps) {
  return reading.kind === "value" ? (
    <SystemReading
      label={label}
      value={reading.value}
      unit={reading.unit}
      wide={wide}
      stale={stale}
    />
  ) : (
    <SystemReading
      label={label}
      value={null}
      wide={wide}
      {...(reading.why === undefined ? {} : { missingWhy: reading.why })}
    />
  );
}

function kick(reading: Pending<{ readonly speedKmS: number }>): Shown {
  return pending(reading, (natal) => shown(formatSignificant(natal.speedKmS), "km/s"));
}

/** The rows only a dead star has: what is known of what it left. */
function remnantRows(remnant: HostRemnant, stale: boolean): ReactNode {
  let rows: ReactNode;
  switch (remnant.kind) {
    case "white_dwarf": {
      const age = formatAge(remnant.coolingAgeMyr);
      rows = (
        <>
          <Reading label="COOLING AGE" reading={shown(age.value, age.unit)} stale={stale} />
          <Reading label="KICK" reading={kick(remnant.natalKick)} stale={stale} />
        </>
      );
      break;
    }
    case "neutron_star":
      rows = (
        <>
          <Reading
            label="PULSAR PERIOD"
            reading={pending(remnant.pulsar, (pulsar) =>
              shown(formatSignificant(pulsar.spinPeriodS), "s"),
            )}
            stale={stale}
          />
          <Reading label="KICK" reading={kick(remnant.natalKick)} stale={stale} />
        </>
      );
      break;
    case "black_hole":
      rows = (
        <>
          <Reading
            label="SPIN"
            reading={pending(remnant.dimensionlessSpin, (spin) => shown(formatNumber(spin, 3)))}
            stale={stale}
          />
          <Reading label="KICK" reading={kick(remnant.natalKick)} stale={stale} />
        </>
      );
      break;
    case "no_remnant":
      rows = null;
      break;
  }
  return rows;
}

/** Seconds from `from` to `to`, whole seconds and nanoseconds taken apart. */
function secondsBetween(from: UniverseTime, to: UniverseTime): number {
  return to.seconds - from.seconds + (to.nanos - from.nanos) / NANOS_PER_SECOND;
}

/**
 * The death inside the clock window as a reading: `DIES IN 312 yr` ahead of the time shown, `DIED`
 * `312 yr AGO` before it; `null` when the death is outside the window.
 */
function deathRow(deathTime: UniverseTime | null, at: UniverseTime, stale: boolean): ReactNode {
  if (deathTime === null) {
    return null;
  }
  const deltaS = secondsBetween(at, deathTime);
  const period = formatPeriod(Math.abs(deltaS) / SECONDS_PER_DAY);
  return deltaS > 0 ? (
    <SystemReading label="DIES IN" value={period.value} unit={period.unit} stale={stale} />
  ) : (
    <SystemReading
      label="DIED"
      value={period.value}
      unit={period.unit}
      beside="AGO"
      stale={stale}
    />
  );
}

/** The events in progress, in words: `FLARE, THERMAL PULSE`, or `NONE`. */
function eventsShown(events: Pending<ReadonlyArray<StarEvent>>): Shown {
  return pending(events, (list) =>
    shown(list.length === 0 ? "NONE" : list.map((event) => starEventLabel(event.kind)).join(", ")),
  );
}

/** Props of {@link PrimaryReadings}. */
export interface PrimaryReadingsProps {
  /** The system's primary, as `system_summary` described it. */
  readonly host: HostBody;
  /** The time the summary describes, from which a death is counted. */
  readonly at: UniverseTime;
  /** Whether the answer is a stale snapshot, for a time the chart has left or a failed request. */
  readonly stale: boolean;
}

/**
 * The selected system's primary star as `system_summary` describes it, for the `GALAXY` display's
 * readout (plan 06, P06.T36).
 *
 * @remarks
 * The same readings the `SYSTEM` display's host readout gives, in the same words and units: its
 * kind, phase and class in words; its luminosity in `L☉` and radius in `R☉`, or in `km` for a
 * neutron star or a black hole (ruling 36); its effective temperature in `K` and its absolute visual
 * magnitude, `M(V)`, in `mag`; what this generator version does not model yet (rotation, variability,
 * the nebula, the events in progress, a pulsar's and a black hole's spin, kicks) is the guide's em
 * dash, and what is modelled as none reads `NONE`; an object with no light has the em dash with the
 * reason, `NO LIGHT`. Dead, it reads what it left and what is known of it; a star that left no
 * remnant has nothing whose light or size could be read, so those rows are left out. A death inside
 * the clock window reads `DIES IN 312 yr`, or `DIED 312 yr AGO` for one before the time shown. Its
 * mass now stands in the system's rows beside its initial mass. A stale answer's values are muted
 * and trail the guide's `S`.
 */
export function PrimaryReadings({ host, at, stale }: PrimaryReadingsProps) {
  const gone = host.kind === "no_remnant";
  const radius =
    host.kind === "neutron_star" || host.kind === "black_hole"
      ? shown(formatRadiusKm(host.radiusRsun * KM_PER_RSUN), "km")
      : shown(formatRadiusRsun(host.radiusRsun), <SolarUnit quantity="radius" />);
  return (
    <>
      <Reading label="KIND" reading={shown(objectKindLabel(host.kind))} stale={stale} wide />
      <Reading label="PHASE" reading={shown(phaseLabel(host.phase))} stale={stale} wide />
      <Reading label="CLASS" reading={shown(host.spectralClass)} stale={stale} />
      {deathRow(host.deathTime, at, stale)}
      {gone ? null : (
        <>
          <Reading
            label="LUM"
            reading={
              host.teffK === null
                ? NO_LIGHT_SHOWN
                : shown(
                    formatLuminosityLsun(host.luminosityLsun),
                    <SolarUnit quantity="luminosity" />,
                  )
            }
            stale={stale}
          />
          <Reading label="RADIUS" reading={radius} stale={stale} />
          <Reading
            label="T EFF"
            reading={
              host.teffK === null ? NO_LIGHT_SHOWN : shown(formatTemperatureK(host.teffK), "K")
            }
            stale={stale}
          />
          <Reading
            label="M(V)"
            reading={
              host.absoluteVMag === null
                ? MISSING
                : shown(formatSigned(host.absoluteVMag, MAGNITUDE_DECIMALS), "mag")
            }
            stale={stale}
          />
        </>
      )}
      {host.remnant === null ? null : (
        <>
          <Reading label="REMNANT" reading={shown(remnantLabel(host.remnant))} stale={stale} />
          {remnantRows(host.remnant, stale)}
        </>
      )}
      {gone ? null : (
        <>
          <Reading
            label="ROTATION"
            reading={modelled(host.rotationPeriodD, (periodD) => {
              const period = formatPeriod(periodD);
              return shown(period.value, period.unit);
            })}
            stale={stale}
          />
          <Reading
            label="VARIABILITY"
            reading={modelled(host.variability, (variability) =>
              shown(variableKindLabel(variability.kind)),
            )}
            stale={stale}
          />
          <Reading
            label="NEBULA RADIUS"
            reading={modelled(host.planetaryNebula, (nebula) =>
              shown(formatSignificant(nebula.radiusLy), "ly"),
            )}
            stale={stale}
          />
          <Reading label="EVENTS" reading={eventsShown(host.activeEvents)} stale={stale} wide />
        </>
      )}
    </>
  );
}
