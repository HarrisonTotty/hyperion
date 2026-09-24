import type { ReactNode } from "react";

import { SolarMassUnit } from "../../components/SolarMassUnit";
import { SolarUnit } from "../../components/SolarUnit";
import {
  formatAge,
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
import type { HostBody, HostRemnant, Modelled, Pending } from "../../lib/system/model";
import {
  objectKindLabel,
  phaseLabel,
  remnantLabel,
  variableKindLabel,
} from "../../lib/system/words";
import { formatBodyIdHex } from "../../lib/seed";

/** What a reading shows: a value with its unit, a value in words, or the em dash with why. */
type Shown =
  | { readonly kind: "value"; readonly value: string; readonly unit?: ReactNode }
  | { readonly kind: "missing"; readonly why?: string };

interface ReadingProps {
  readonly label: ReactNode;
  readonly shown: Shown;
  /** Whether the value takes the rest of the row, as a designation does. */
  readonly wide?: boolean;
}

function Reading({ label, shown, wide = false }: ReadingProps) {
  return (
    <>
      <dt>{label}</dt>
      <dd className={wide ? "body-readout__wide" : undefined}>
        {shown.kind === "missing" ? (
          <>
            <span className="readout__missing">—</span>
            {shown.why === undefined ? null : (
              <span className="body-readout__why">{shown.why}</span>
            )}
          </>
        ) : (
          <>
            <span className="body-readout__value">{shown.value}</span>
            {shown.unit === undefined ? null : (
              <>
                {" "}
                <span className="body-readout__unit">{shown.unit}</span>
              </>
            )}
          </>
        )}
      </dd>
    </>
  );
}

const MISSING: Shown = { kind: "missing" };

/** Why a light-less object has no luminosity or temperature, in the readout's words. */
const NO_LIGHT = "NO LIGHT";

function value(text: string, unit?: ReactNode): Shown {
  return unit === undefined ? { kind: "value", value: text } : { kind: "value", value: text, unit };
}

/** A value this generator version may not model: the em dash, `NONE`, or the value. */
function modelled<T>(reading: Modelled<T>, show: (present: T) => Shown): Shown {
  let shown: Shown;
  switch (reading.kind) {
    case "not_modelled":
      shown = MISSING;
      break;
    case "none":
      shown = value("NONE");
      break;
    case "value":
      shown = show(reading.value);
      break;
  }
  return shown;
}

/** A value the wire leaves out until its task lands: the em dash, or the value. */
function pending<T>(reading: Pending<T>, show: (present: T) => Shown): Shown {
  return reading.kind === "not_modelled" ? MISSING : show(reading.value);
}

/** Whether a host's radius is read in kilometres: a neutron star's or a black hole's (ruling 36). */
function radiusInKm(host: HostBody): boolean {
  return host.kind === "neutron_star" || host.kind === "black_hole";
}

/** The rows only a dead star has: what it left and what is known of it. */
function remnantReadings(remnant: HostRemnant): ReactNode {
  let rows: ReactNode;
  switch (remnant.kind) {
    case "white_dwarf": {
      const age = formatAge(remnant.coolingAgeMyr);
      rows = (
        <>
          <Reading label="COOLING AGE" shown={value(age.value, age.unit)} />
          <Reading
            label="KICK"
            shown={pending(remnant.natalKick, (kick) =>
              value(formatSignificant(kick.speedKmS), "km/s"),
            )}
          />
        </>
      );
      break;
    }
    case "neutron_star":
      rows = (
        <>
          <Reading
            label="PULSAR PERIOD"
            shown={pending(remnant.pulsar, (pulsar) =>
              value(formatSignificant(pulsar.spinPeriodS), "s"),
            )}
          />
          <Reading
            label="KICK"
            shown={pending(remnant.natalKick, (kick) =>
              value(formatSignificant(kick.speedKmS), "km/s"),
            )}
          />
        </>
      );
      break;
    case "black_hole":
      rows = (
        <>
          <Reading
            label="SPIN"
            shown={pending(remnant.dimensionlessSpin, (spin) => value(formatNumber(spin, 3)))}
          />
          <Reading
            label="KICK"
            shown={pending(remnant.natalKick, (kick) =>
              value(formatSignificant(kick.speedKmS), "km/s"),
            )}
          />
        </>
      );
      break;
    case "no_remnant":
      rows = null;
      break;
  }
  return rows;
}

/** Props of {@link BodyReadout}. */
export interface BodyReadoutProps {
  /** The selected body, or `null` when none is. */
  readonly host: HostBody | null;
}

/**
 * Everything the server says of the selected body: for a host, its star as `system_summary`
 * describes it (plan 14, P14.T43.b; plan 06, P06.T36).
 *
 * @remarks
 * A live region read as a whole, so that a selection made on the map or in the list is announced
 * once. It is a `div` with `role="status"`, not an `output`, since it holds a `dl` (the
 * orchestrator's ruling 14). Every number is the server's (D18). A host reads its kind, phase and
 * class; its initial mass and its mass now in `M☉`; its luminosity in `L☉` and radius in `R☉`,
 * or in `km` for a neutron star or a black hole (ruling 36); its effective temperature in `K`;
 * and, dead, what it left, a white dwarf's cooling age, and each remnant's spin and kick. What
 * this generator version does not model yet (rotation, activity, variability, a pulsar's spin, a
 * black hole's spin, kicks) is the guide's em dash; what is modelled as none reads `NONE`. An
 * object with no light, as a black hole, has the em dash for its luminosity and temperature with
 * the reason, `NO LIGHT`. A star that left no remnant has nothing whose mass, light or size could
 * be read, so those rows are left out, as a section that does not apply is (ruling 34). With
 * nothing selected every value is an em dash.
 */
export function BodyReadout({ host }: BodyReadoutProps) {
  const gone = host?.kind === "no_remnant";
  const noLight = host !== null && host.teffK === null;
  return (
    // `output`'s content model is phrasing content, so it cannot hold this `dl`: a role is added
    // where no native element fits. Atomic, so that the whole selection is read.
    // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
    <div className="body-readout" role="status" aria-label="Selected body" aria-atomic="true">
      <dl className="readout body-readout__values">
        <Reading label="DESIG" shown={host === null ? MISSING : value(host.designation)} wide />
        <Reading
          label="ID"
          shown={host === null ? MISSING : value(formatBodyIdHex(host.id))}
          wide
        />
        <Reading
          label="KIND"
          shown={host === null ? MISSING : value(objectKindLabel(host.kind))}
          wide
        />
        <Reading
          label="PHASE"
          shown={host === null ? MISSING : value(phaseLabel(host.phase))}
          wide
        />
        <Reading label="CLASS" shown={host === null ? MISSING : value(host.spectralClass)} />
        <Reading
          label="INIT MASS"
          shown={
            host === null ? MISSING : value(formatMassMsun(host.initialMassMsun), <SolarMassUnit />)
          }
        />
        {gone ? null : (
          <>
            <Reading
              label="MASS"
              shown={
                host === null ? MISSING : value(formatMassMsun(host.massMsun), <SolarMassUnit />)
              }
            />
            <Reading
              label="LUM"
              shown={
                host === null
                  ? MISSING
                  : noLight
                    ? { kind: "missing", why: NO_LIGHT }
                    : value(
                        formatLuminosityLsun(host.luminosityLsun),
                        <SolarUnit quantity="luminosity" />,
                      )
              }
            />
            <Reading
              label="RADIUS"
              shown={
                host === null
                  ? MISSING
                  : radiusInKm(host)
                    ? value(formatRadiusKm(host.radiusRsun * KM_PER_RSUN), "km")
                    : value(formatRadiusRsun(host.radiusRsun), <SolarUnit quantity="radius" />)
              }
            />
            <Reading
              label="T EFF"
              shown={
                host === null || host.teffK === null
                  ? { kind: "missing", ...(noLight ? { why: NO_LIGHT } : {}) }
                  : value(formatTemperatureK(host.teffK), "K")
              }
            />
          </>
        )}
        {host?.remnant === null || host?.remnant === undefined ? null : (
          <>
            <Reading label="REMNANT" shown={value(remnantLabel(host.remnant))} />
            {remnantReadings(host.remnant)}
          </>
        )}
        {gone ? null : (
          <>
            <Reading
              label="ROTATION"
              shown={
                host === null
                  ? MISSING
                  : modelled(host.rotationPeriodD, (periodD) => {
                      const period = formatPeriod(periodD);
                      return value(period.value, period.unit);
                    })
              }
            />
            <Reading
              label="ACTIVITY"
              shown={
                host === null
                  ? MISSING
                  : modelled(host.activityLogLxLbol, (activity) =>
                      value(formatNumber(activity, 2), "dex"),
                    )
              }
            />
            <Reading
              label="VARIABILITY"
              shown={
                host === null
                  ? MISSING
                  : modelled(host.variability, (variability) =>
                      value(variableKindLabel(variability.kind)),
                    )
              }
              wide
            />
          </>
        )}
      </dl>
    </div>
  );
}
