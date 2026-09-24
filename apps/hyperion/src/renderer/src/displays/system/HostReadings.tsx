import { Fragment, type ReactNode } from "react";

import { SolarMassUnit } from "../../components/SolarMassUnit";
import { SolarUnit } from "../../components/SolarUnit";
import {
  formatAge,
  formatBodyDistance,
  formatLuminosityLsun,
  formatMassMsun,
  formatNumber,
  formatPeriod,
  formatRadiusKm,
  formatRadiusRsun,
  formatSignificant,
  formatTemperatureK,
  KM_PER_RSUN,
} from "../../lib/format";
import { architectureLabel } from "../../lib/system/bodyWords";
import type {
  HabitableZone,
  HostBody,
  HostRemnant,
  Modelled,
  Pending,
  Zone,
} from "../../lib/system/model";
import {
  objectKindLabel,
  phaseLabel,
  remnantLabel,
  variableKindLabel,
} from "../../lib/system/words";
import { formatBodyIdHex } from "../../lib/seed";
import { MISSING, ReadoutRow, type Shown, shown as value } from "./ReadoutRow";
import type { HostZone } from "./useSystemView";

/** Why a light-less object has no luminosity or temperature, in the readout's words. */
const NO_LIGHT = "NO LIGHT";

/** Kilometres in a metre. */
const KM_PER_M = 1e-3;

/** A value this generator version may not model: the em dash, `NONE`, or the value. */
function modelled<T>(reading: Modelled<T>, show: (present: T) => Shown): Shown {
  let result: Shown;
  switch (reading.kind) {
    case "not_modelled":
      result = MISSING;
      break;
    case "none":
      result = value("NONE");
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

/** Whether a host's radius is read in kilometres: a neutron star's or a black hole's (ruling 36). */
function radiusInKm(host: HostBody): boolean {
  return host.kind === "neutron_star" || host.kind === "black_hole";
}

function kick(reading: Pending<{ readonly speedKmS: number }>): Shown {
  return pending(reading, (natal) => value(formatSignificant(natal.speedKmS), "km/s"));
}

/** The rows only a dead star has: what it left and what is known of it. */
function remnantReadings(remnant: HostRemnant): ReactNode {
  let rows: ReactNode;
  switch (remnant.kind) {
    case "white_dwarf": {
      const age = formatAge(remnant.coolingAgeMyr);
      rows = (
        <>
          <ReadoutRow label="COOLING AGE" shown={value(age.value, age.unit)} />
          <ReadoutRow label="KICK" shown={kick(remnant.natalKick)} />
        </>
      );
      break;
    }
    case "neutron_star":
      rows = (
        <>
          <ReadoutRow
            label="PULSAR PERIOD"
            shown={pending(remnant.pulsar, (pulsar) =>
              value(formatSignificant(pulsar.spinPeriodS), "s"),
            )}
          />
          <ReadoutRow label="KICK" shown={kick(remnant.natalKick)} />
        </>
      );
      break;
    case "black_hole":
      rows = (
        <>
          <ReadoutRow
            label="SPIN"
            shown={pending(remnant.dimensionlessSpin, (spin) => value(formatNumber(spin, 3)))}
          />
          <ReadoutRow label="KICK" shown={kick(remnant.natalKick)} />
        </>
      );
      break;
    case "no_remnant":
      rows = null;
      break;
  }
  return rows;
}

/** A distance from a zone's host, with its unit, as one string: `2.26 AU`. */
function zoneDistance(metres: number, estimated: boolean): string {
  const distance = formatBodyDistance(metres * KM_PER_M, null);
  return `${estimated ? "~" : ""}${distance.value} ${distance.unit}`;
}

/**
 * A zone's stable limits, where its companions set them: `NONE` for an edge its disc bounds; `null`
 * when neither is set, as about a single star, and the row is left out.
 */
function stableZone(zone: Zone): string | null {
  if (zone.innerM === null && zone.outerM === null) {
    return null;
  }
  const edge = (metres: number | null): string =>
    metres === null ? "NONE" : zoneDistance(metres, false);
  return `${edge(zone.innerM)} – ${edge(zone.outerM)}`;
}

/**
 * One pair of a habitable zone's limits as a span: `NONE` where there is none (no light, or an
 * inner edge beyond every orbit), `FROM` its inner edge where its outer edge lies beyond every
 * orbit; each limit `~` where the host's temperature lay outside the fit's range.
 */
function habitableSpan(
  habitable: HabitableZone | null,
  edges: (zone: HabitableZone) => readonly [number | null, number | null],
): string {
  if (habitable === null) {
    return "NONE";
  }
  const [innerM, outerM] = edges(habitable);
  if (innerM === null || !(innerM > 0)) {
    return "NONE";
  }
  const inner = zoneDistance(innerM, habitable.extrapolated);
  return outerM === null
    ? `FROM ${inner}`
    : `${inner} – ${zoneDistance(outerM, habitable.extrapolated)}`;
}

/** A zone's conservative habitable zone, from the moist greenhouse to the maximum greenhouse. */
function habitableZone(zone: Zone): string {
  return habitableSpan(zone.habitableZone, (hz) => [hz.moistGreenhouseM, hz.maximumGreenhouseM]);
}

/**
 * A zone's optimistic habitable zone, from recent Venus to early Mars, the same fit's other pair of
 * limits (ruling 65.4).
 */
function optimisticZone(zone: Zone): string {
  return habitableSpan(zone.habitableZone, (hz) => [hz.recentVenusM, hz.earlyMarsM]);
}

/** The rows of one zone that holds a host. */
function zoneReadings({ zone, about }: HostZone): ReactNode {
  const stable = stableZone(zone);
  return (
    <>
      <ReadoutRow label="ZONE" shown={value(about)} wide />
      <ReadoutRow label="ARCH" shown={value(architectureLabel(zone.architecture))} wide />
      {stable === null ? null : <ReadoutRow label="STABLE ZONE" shown={value(stable)} wide />}
      <ReadoutRow label="SNOW LINE" shown={value(zoneDistance(zone.snowLineM, false))} wide />
      <ReadoutRow label="HABITABLE ZONE" shown={value(habitableZone(zone))} wide />
      <ReadoutRow label="OPTIMISTIC" shown={value(optimisticZone(zone))} wide />
    </>
  );
}

/** Props of {@link HostReadings}. */
export interface HostReadingsProps {
  readonly host: HostBody;
  /** The zones that hold it, innermost first. */
  readonly zones: ReadonlyArray<HostZone>;
}

/**
 * A host's readings: its star as `system_summary` describes it, and the zones that hold it (plan 14,
 * P14.T43.b; plan 06, P06.T36).
 *
 * @remarks
 * Every number is the server's (D18). A host reads its kind, phase and class; its initial mass and
 * its mass now in `M☉`; its luminosity in `L☉` and radius in `R☉`, or in `km` for a neutron star or
 * a black hole (ruling 36); its effective temperature in `K`; and, dead, what it left, a white
 * dwarf's cooling age, and each remnant's spin and kick. What this generator version does not model
 * yet (rotation, activity, variability, a pulsar's spin, a black hole's spin, kicks) is the guide's
 * em dash; what is modelled as none reads `NONE`. An object with no light, as a black hole, has the
 * em dash for its luminosity and temperature with the reason, `NO LIGHT`. A star that left no
 * remnant has nothing whose mass, light or size could be read, so those rows are left out, as a
 * section that does not apply is (ruling 34). Then each zone that holds it, its own first: what the
 * zone is about, its host's architecture class, its stable limits where companions set them, its
 * snow line, its conservative habitable zone and its optimistic one (`OPTIMISTIC`, ruling 65.4).
 */
export function HostReadings({ host, zones }: HostReadingsProps) {
  const gone = host.kind === "no_remnant";
  const noLight = host.teffK === null;
  return (
    <>
      <ReadoutRow label="DESIG" shown={value(host.designation)} wide />
      <ReadoutRow label="ID" shown={value(formatBodyIdHex(host.id))} wide />
      <ReadoutRow label="KIND" shown={value(objectKindLabel(host.kind))} wide />
      <ReadoutRow label="PHASE" shown={value(phaseLabel(host.phase))} wide />
      <ReadoutRow label="CLASS" shown={value(host.spectralClass)} />
      <ReadoutRow
        label="INIT MASS"
        shown={value(formatMassMsun(host.initialMassMsun), <SolarMassUnit />)}
      />
      {gone ? null : (
        <>
          <ReadoutRow
            label="MASS"
            shown={value(formatMassMsun(host.massMsun), <SolarMassUnit />)}
          />
          <ReadoutRow
            label="LUM"
            shown={
              noLight
                ? { kind: "missing", why: NO_LIGHT }
                : value(
                    formatLuminosityLsun(host.luminosityLsun),
                    <SolarUnit quantity="luminosity" />,
                  )
            }
          />
          <ReadoutRow
            label="RADIUS"
            shown={
              radiusInKm(host)
                ? value(formatRadiusKm(host.radiusRsun * KM_PER_RSUN), "km")
                : value(formatRadiusRsun(host.radiusRsun), <SolarUnit quantity="radius" />)
            }
          />
          <ReadoutRow
            label="T EFF"
            shown={
              host.teffK === null
                ? { kind: "missing", why: NO_LIGHT }
                : value(formatTemperatureK(host.teffK), "K")
            }
          />
        </>
      )}
      {host.remnant === null ? null : (
        <>
          <ReadoutRow label="REMNANT" shown={value(remnantLabel(host.remnant))} />
          {remnantReadings(host.remnant)}
        </>
      )}
      {gone ? null : (
        <>
          <ReadoutRow
            label="ROTATION"
            shown={modelled(host.rotationPeriodD, (periodD) => {
              const period = formatPeriod(periodD);
              return value(period.value, period.unit);
            })}
          />
          <ReadoutRow
            label="ACTIVITY"
            shown={modelled(host.activityLogLxLbol, (activity) =>
              value(formatNumber(activity, 2), "dex"),
            )}
          />
          <ReadoutRow
            label="VARIABILITY"
            shown={modelled(host.variability, (variability) =>
              value(variableKindLabel(variability.kind)),
            )}
            wide
          />
        </>
      )}
      {zones.map((zone) => (
        <Fragment key={zone.about}>{zoneReadings(zone)}</Fragment>
      ))}
    </>
  );
}
