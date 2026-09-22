import type { ParameterGroup } from "@hyperion/protocol";
import { useId, useState } from "react";

import { RequestStatus } from "../../components/RequestStatus";
import { UnitLabel } from "../../components/UnitLabel";
import { formatListPosition, formatNumber } from "../../lib/format";
import { formatHex64 } from "../../lib/seed";
import { useUniverse } from "../../lib/universe";
import { type ScrollMetrics, useScrollMetrics } from "../../lib/useScrollMetrics";
import { REQUEST_TIMEOUT_MS, useServerRequest } from "../../lib/useServerRequest";
import { windowRange } from "../../lib/windowRange";
import { groupLabel } from "./parameterLabels";
import { type ParameterReading, toReading } from "./parameterReadings";

/**
 * The list's layout unit: a group heading is one unit tall and a parameter row, whose label sits
 * above its value, two.
 */
const LINE_UNIT_REM = 1.5;
const ROW_UNITS = 2;

/**
 * Keys of the parameters shown above the list, from the response's own fields, and so left out
 * of their group.
 */
const SHOWN_ABOVE = new Set(["seed", "generator_version"]);

interface GroupReadings {
  readonly key: string;
  readonly label: string;
  readonly readings: ReadonlyArray<ParameterReading>;
}

function toGroups(groups: ReadonlyArray<ParameterGroup>): GroupReadings[] {
  return groups
    .map((group) => ({
      key: group.key,
      label: groupLabel(group.key),
      readings: group.parameters
        .filter((parameter) => !SHOWN_ABOVE.has(parameter.key))
        .map(toReading),
    }))
    .filter((group) => group.readings.length > 0);
}

/**
 * The parameter each layout unit of the list belongs to, or −1 for a group heading, in list
 * order.
 */
function parameterOfUnit(groups: ReadonlyArray<GroupReadings>): number[] {
  const units: number[] = [];
  let parameter = 0;
  for (const group of groups) {
    units.push(-1);
    for (let row = 0; row < group.readings.length; row += 1) {
      for (let unit = 0; unit < ROW_UNITS; unit += 1) {
        units.push(parameter);
      }
      parameter += 1;
    }
  }
  return units;
}

/** The parameters at least partly in view, as `12-24 of 87`, or `null` for an empty list. */
function positionText(groups: ReadonlyArray<GroupReadings>, metrics: ScrollMetrics): string | null {
  const units = parameterOfUnit(groups);
  const total = groups.reduce((count, group) => count + group.readings.length, 0);
  const range = windowRange(
    metrics.scrollTopPx,
    LINE_UNIT_REM * metrics.remPx,
    metrics.viewportPx,
    units.length,
    0,
  );
  if (range === null || total === 0) {
    return null;
  }
  const inView = units
    .slice(range.firstVisible, range.lastVisible + 1)
    .filter((parameter) => parameter >= 0);
  // With only a heading in view, the next parameter is the one the operator is looking at.
  const first = inView[0] ?? units.slice(range.firstVisible).find((parameter) => parameter >= 0);
  const last = inView.at(-1) ?? first;
  return first === undefined || last === undefined ? null : formatListPosition(first, last, total);
}

interface ParameterTableProps {
  readonly group: GroupReadings;
}

function ParameterTable({ group }: ParameterTableProps) {
  const headingId = useId();
  return (
    <div className="parameters__group">
      <h3 className="parameters__group-title" id={headingId}>
        {group.label}
      </h3>
      <table className="parameter-table" aria-labelledby={headingId}>
        <tbody>
          {group.readings.map((reading) => (
            <tr key={reading.key}>
              <th scope="row" className="parameter-table__label">
                {reading.label}
              </th>
              <td className="parameter-table__value">{reading.text}</td>
              <td className="parameter-table__unit">
                {reading.unit === null ? null : <UnitLabel unit={reading.unit} />}
              </td>
              <td className="parameter-table__origin">{reading.origin}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

interface ParameterListProps {
  readonly groups: ReadonlyArray<GroupReadings>;
}

/** Every group's table, scrolling inside the panel, with the position of the rows in view. */
function ParameterList({ groups }: ParameterListProps) {
  const { ref, metrics } = useScrollMetrics();
  const position = positionText(groups, metrics);
  return (
    <>
      <section
        className="parameters__list"
        ref={ref}
        aria-label="Galaxy parameters"
        // A scrolling list with nothing focusable in it must take focus itself to be scrolled from
        // the keyboard (WCAG 2.1.1); the rule allows tabIndex on interactive roles only.
        // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
      >
        {groups.map((group) => (
          <ParameterTable key={group.key} group={group} />
        ))}
      </section>
      {position === null ? null : <p className="list-position">{position}</p>}
    </>
  );
}

/**
 * The `PARAMETERS` page: the open universe's seed, generator version and drawn structural
 * parameters, grouped, each with its unit and whether it was drawn, derived or fixed.
 *
 * @remarks
 * A page of the panel it shares with `GALAXY MAP`, whose page selector names it. Requested for the
 * open universe, and again whenever another is opened or the operator asks with `RETRY` after a
 * failure. Labels come from the client's glossary, and a key it lacks is shown as itself (plan 05,
 * design note D8a). Values are right-aligned in B612 Mono with their units beside them; nothing is
 * converted. All rows stay in the DOM, since there are a few dozen, and the list scrolls inside the
 * page, from the keyboard too, with the position of the rows in view beneath it.
 */
export function ParametersPanel() {
  const { open } = useUniverse();
  const [retries, setRetries] = useState(0);
  const state = useServerRequest<"galaxy_parameters">(
    open === null ? null : { kind: "galaxy_parameters", universe: open.id },
    REQUEST_TIMEOUT_MS,
    retries,
  );

  return (
    <div className="parameters">
      {open === null ? <p className="panel__empty">NO UNIVERSE OPEN</p> : null}
      <RequestStatus
        state={state}
        onRetry={() => {
          setRetries((count) => count + 1);
        }}
      />
      {state.kind === "ok" ? (
        <>
          <dl className="readout parameters__identity">
            <dt>SEED</dt>
            <dd>{formatHex64(state.response.seed)}</dd>
            <dt>GEN VER</dt>
            <dd>{formatNumber(state.response.generator_version, 0)}</dd>
          </dl>
          <ParameterList groups={toGroups(state.response.groups)} />
        </>
      ) : null}
    </div>
  );
}
