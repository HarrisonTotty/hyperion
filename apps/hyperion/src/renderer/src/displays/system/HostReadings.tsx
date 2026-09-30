import { Fragment, type ReactNode, useState } from "react";

import { SolarMassUnit } from "../../components/SolarMassUnit";
import { SolarUnit } from "../../components/SolarUnit";
import {
  type BodyDistanceUnit,
  formatAge,
  formatBodyDistance,
  formatBodySpan,
  formatLuminosityLsun,
  formatMassMsun,
  formatNumber,
  formatPeriod,
  formatRadiusKm,
  formatRadiusRsun,
  formatSignificant,
  formatTemperatureK,
  KM_PER_RSUN,
  SPAN_JOIN,
} from "../../lib/format";
import { architectureLabel } from "../../lib/system/bodyWords";
import type {
  HabitableZone,
  HostBody,
  HostRemnant,
  Modelled,
  Pending,
  PlanetaryNebula,
  StarEvent,
  Zone,
} from "../../lib/system/model";
import {
  objectKindLabel,
  phaseLabel,
  remnantLabel,
  starEventLabel,
  variableKindLabel,
} from "../../lib/system/words";
import { formatBodyIdHex } from "../../lib/seed";
import { BodyIdRow } from "./BodyIdRow";
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

/** Decimals of the nebula's expansion speed, as a speed in km/s is read (`VEL`). */
const EXPANSION_DECIMALS = 1;

/**
 * The planetary nebula's rows (the orchestrator's ruling 149.4): `NEBULA RADIUS` always, the em dash
 * where it is not modelled and `NONE` where the star lights none; with a nebula, its age, expansion
 * speed, ionised mass and excitation class, which a class on a scale reads bare, as a spectral
 * class does.
 */
function nebulaReadings(nebula: Modelled<PlanetaryNebula>): ReactNode {
  return (
    <>
      <ReadoutRow
        label="NEBULA RADIUS"
        shown={modelled(nebula, (shell) => value(formatSignificant(shell.radiusLy), "ly"))}
      />
      {nebula.kind === "value" ? (
        <>
          <ReadoutRow
            label="NEBULA AGE"
            shown={value(formatSignificant(nebula.value.ageYr), "yr")}
          />
          <ReadoutRow
            label="NEBULA EXPANSION"
            shown={value(formatNumber(nebula.value.expansionSpeedKmS, EXPANSION_DECIMALS), "km/s")}
          />
          <ReadoutRow
            label="IONISED MASS"
            shown={value(formatSignificant(nebula.value.ionisedMassMsun), <SolarMassUnit />)}
          />
          <ReadoutRow
            label="EXCITATION CLASS"
            shown={value(formatNumber(nebula.value.excitationClass, 0))}
          />
        </>
      ) : null}
    </>
  );
}

/** The events in progress, in the `GALAXY` readout's words: `FLARE, THERMAL PULSE`, or `NONE`. */
function eventsShown(events: Pending<ReadonlyArray<StarEvent>>): Shown {
  return pending(events, (list) =>
    value(list.length === 0 ? "NONE" : list.map((event) => starEventLabel(event.kind)).join(", ")),
  );
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
function zoneDistance(metres: number): string {
  const distance = formatBodyDistance(metres * KM_PER_M, null);
  return `${distance.value} ${distance.unit}`;
}

/**
 * A zone's limits as the readout reads them, before they are written: both ends, one end with the
 * other `NONE` (a stable zone's side its disc bounds), `FROM` an inner end whose outer end lies
 * beyond every orbit (a habitable zone), or `NONE` for the whole reading.
 */
type ZoneSpan =
  | { readonly kind: "none" }
  | { readonly kind: "span"; readonly innerM: number; readonly outerM: number }
  | { readonly kind: "inner_only"; readonly innerM: number }
  | { readonly kind: "outer_only"; readonly outerM: number }
  | { readonly kind: "from"; readonly innerM: number };

/**
 * A zone span written as one reading (ruling 113.3), with the unit it was written in, which the row
 * holds for its hysteresis; `null` where no distance is written.
 *
 * @remarks
 * Two ends are one reading with one unit, the outer end's, in the row's unit slot: `0.989 – 1.69`
 * `AU`. `FROM` an inner end is one distance, its unit in the slot too: `FROM 0.950` `AU`. Where one
 * end is `NONE`, the unit stays with the one number beside it, `NONE – 1.69 AU`, so that the slot
 * never reads `NONE AU`. An estimated zone, one extrapolated past the fit's temperatures, marks each
 * number it writes `~`, since each is estimated (the UX reviewer's forms, as ui12 was told).
 *
 * @param held - The unit the row last showed; `null` for its first showing.
 */
function writeZoneSpan(
  span: ZoneSpan,
  estimated: boolean,
  held: BodyDistanceUnit | null,
): { readonly shown: Shown; readonly unit: BodyDistanceUnit | null } {
  const mark = estimated ? "~" : "";
  let result: { readonly shown: Shown; readonly unit: BodyDistanceUnit | null };
  switch (span.kind) {
    case "none":
      result = { shown: value("NONE"), unit: null };
      break;
    case "span": {
      const written = formatBodySpan(span.innerM * KM_PER_M, span.outerM * KM_PER_M, held);
      // Each end marked where the zone is estimated: `~0.950 – ~1.68`.
      const ends = written.value.split(SPAN_JOIN).map((end) => `${mark}${end}`);
      result = { shown: value(ends.join(SPAN_JOIN), written.unit), unit: written.unit };
      break;
    }
    case "from": {
      const at = formatBodyDistance(span.innerM * KM_PER_M, held);
      result = { shown: value(`FROM ${mark}${at.value}`, at.unit), unit: at.unit };
      break;
    }
    case "inner_only": {
      const at = formatBodyDistance(span.innerM * KM_PER_M, held);
      result = {
        shown: value(`${mark}${at.value} ${at.unit}${SPAN_JOIN}NONE`),
        unit: at.unit,
      };
      break;
    }
    case "outer_only": {
      const at = formatBodyDistance(span.outerM * KM_PER_M, held);
      result = {
        shown: value(`NONE${SPAN_JOIN}${mark}${at.value} ${at.unit}`),
        unit: at.unit,
      };
      break;
    }
  }
  return result;
}

/** Props of {@link ZoneSpanRow}. */
interface ZoneSpanRowProps {
  readonly label: string;
  readonly span: ZoneSpan;
  /** Whether each limit is estimated, the host's temperature lying outside the fit's range. */
  readonly estimated: boolean;
}

/**
 * A zone's limits as one reading, its unit held from one reading to the next with the distance
 * units' hysteresis, adjusted during render as a belt's `EDGES` row is (ruling 113.3).
 */
function ZoneSpanRow({ label, span, estimated }: ZoneSpanRowProps) {
  const [held, setHeld] = useState<BodyDistanceUnit | null>(
    () => writeZoneSpan(span, estimated, null).unit,
  );
  const written = writeZoneSpan(span, estimated, held);
  if (written.unit !== null && written.unit !== held) {
    setHeld(written.unit);
  }
  return <ReadoutRow label={label} shown={written.shown} wide />;
}

/**
 * A zone's stable limits, where its companions set them, with `NONE` for an edge its disc bounds;
 * `null` when neither is set, as about a single star, and the row is left out.
 */
function stableSpan(zone: Zone): ZoneSpan | null {
  const { innerM, outerM } = zone;
  if (innerM === null && outerM === null) {
    return null;
  }
  if (innerM === null) {
    return outerM === null ? null : { kind: "outer_only", outerM };
  }
  return outerM === null ? { kind: "inner_only", innerM } : { kind: "span", innerM, outerM };
}

/**
 * One pair of a habitable zone's limits as a span: `NONE` where there is none (no light, or an
 * inner edge beyond every orbit), `FROM` its inner edge where its outer edge lies beyond every
 * orbit.
 */
function habitableSpan(
  habitable: HabitableZone | null,
  edges: (zone: HabitableZone) => readonly [number | null, number | null],
): ZoneSpan {
  if (habitable === null) {
    return { kind: "none" };
  }
  const [innerM, outerM] = edges(habitable);
  if (innerM === null || !(innerM > 0)) {
    return { kind: "none" };
  }
  return outerM === null ? { kind: "from", innerM } : { kind: "span", innerM, outerM };
}

/** The rows of one zone that holds a host. */
function zoneReadings({ zone, about }: HostZone): ReactNode {
  const stable = stableSpan(zone);
  const estimated = zone.habitableZone?.extrapolated ?? false;
  return (
    <>
      <ReadoutRow label="ZONE" shown={value(about)} wide />
      <ReadoutRow label="ARCH" shown={value(architectureLabel(zone.architecture))} wide />
      {stable === null ? null : <ZoneSpanRow label="STABLE ZONE" span={stable} estimated={false} />}
      <ReadoutRow label="SNOW LINE" shown={value(zoneDistance(zone.snowLineM))} wide />
      {/* The conservative zone, from the moist greenhouse to the maximum greenhouse. */}
      <ZoneSpanRow
        label="HABITABLE ZONE"
        span={habitableSpan(zone.habitableZone, (hz) => [
          hz.moistGreenhouseM,
          hz.maximumGreenhouseM,
        ])}
        estimated={estimated}
      />
      {/* The optimistic zone, from recent Venus to early Mars, the fit's other pair (ruling 65.4). */}
      <ZoneSpanRow
        label="OPTIMISTIC"
        span={habitableSpan(zone.habitableZone, (hz) => [hz.recentVenusM, hz.earlyMarsM])}
        estimated={estimated}
      />
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
 * yet (rotation, activity, variability, the nebula, the events in progress, a pulsar's spin, a black
 * hole's spin, kicks) is the guide's em dash; what is modelled as none reads `NONE`. After
 * `VARIABILITY` stand the planetary nebula's rows, its radius always and the rest only with a
 * nebula, and `EVENTS` (ruling 149.4). An object with no light, as a black hole, has the
 * em dash for its luminosity and temperature with the reason, `NO LIGHT`. A star that left no
 * remnant has nothing whose mass, light or size could be read, so those rows are left out, as a
 * section that does not apply is (ruling 34). Then each zone that holds it, its own first: what the
 * zone is about, its host's architecture class, its stable limits where companions set them, its
 * snow line, its conservative habitable zone and its optimistic one (`OPTIMISTIC`, ruling 65.4),
 * each span one reading with one unit (ruling 113.3).
 */
export function HostReadings({ host, zones }: HostReadingsProps) {
  const gone = host.kind === "no_remnant";
  const noLight = host.teffK === null;
  return (
    <>
      <ReadoutRow label="DESIG" shown={value(host.designation)} wide />
      <BodyIdRow text={formatBodyIdHex(host.id)} />
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
          {nebulaReadings(host.planetaryNebula)}
          <ReadoutRow label="EVENTS" shown={eventsShown(host.activeEvents)} wide />
        </>
      )}
      {zones.map((zone) => (
        <Fragment key={zone.about}>{zoneReadings(zone)}</Fragment>
      ))}
    </>
  );
}
