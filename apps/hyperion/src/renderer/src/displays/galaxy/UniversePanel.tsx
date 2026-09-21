import type { UniverseIdHex, UniverseInfo } from "@hyperion/protocol";
import { useId, useState } from "react";

import { RequestStatus } from "../../components/RequestStatus";
import { formatListPosition, formatNumber } from "../../lib/format";
import { formatHex64 } from "../../lib/seed";
import { linkDownReason, useServerLink } from "../../lib/serverLink";
import { useUniverse } from "../../lib/universe";
import { useScrollMetrics } from "../../lib/useScrollMetrics";
import type { RequestState } from "../../lib/useServerRequest";
import { windowRange } from "../../lib/windowRange";
import { NewUniverseForm } from "./NewUniverseForm";

/** Height of each row: two lines, the first the `OPEN` button's 2rem target. */
const ROW_HEIGHT_REM = 3.5;

/** Height of the table's header: two lines of text, with no command in them. */
const HEADER_HEIGHT_REM = 2.75;

/** Which command the operator last gave from this panel, so that its result shows beside it. */
type LastCommand =
  { readonly kind: "open"; readonly universe: UniverseIdHex } | { readonly kind: "create" } | null;

/**
 * The operator's choice to show or fold the `NEW UNIVERSE` fields, and the open universe it was
 * made for: a choice lasts until another universe is opened.
 */
interface FormChoice {
  readonly openId: UniverseIdHex | null;
  readonly expanded: boolean;
}

/** Why a universe's `OPEN` is held back while nothing else holds it back, or `null`. */
function mismatchReason(universe: UniverseInfo, serverGeneratorVersion: number): string | null {
  return universe.status === "generator_mismatch"
    ? `GENERATOR VERSION ${formatNumber(universe.generator_version, 0)}: server runs version ` +
        formatNumber(serverGeneratorVersion, 0)
    : null;
}

/** The IDs present in `ids`, as an `aria-describedby` value, or `undefined` for none. */
function describedBy(ids: ReadonlyArray<string | null>): string | undefined {
  const present = ids.filter((id) => id !== null);
  return present.length > 0 ? present.join(" ") : undefined;
}

interface ReadingProps {
  readonly label: string;
  /** The value, or `null` for the em dash of a missing value. */
  readonly value: string | null;
  /** Whether the value takes the rest of its line, as a 16-digit seed or ID must. */
  readonly wide?: boolean;
}

function Reading({ label, value, wide = false }: ReadingProps) {
  const className = wide ? "readout__wide" : undefined;
  return (
    <>
      <dt>{label}</dt>
      {value === null ? (
        <dd className={wide ? "readout__wide readout__missing" : "readout__missing"}>—</dd>
      ) : (
        <dd className={className}>{value}</dd>
      )}
    </>
  );
}

interface OpenUniverseReadoutProps {
  readonly open: UniverseInfo | null;
}

/**
 * The open universe: name and generator version on one line, then seed and ID, or em dashes when
 * none is open.
 */
function OpenUniverseReadout({ open }: OpenUniverseReadoutProps) {
  return (
    <>
      {open === null ? <p className="panel__empty">NO UNIVERSE OPEN</p> : null}
      <dl className="readout universe-panel__readout">
        <Reading label="NAME" value={open?.name ?? null} />
        <Reading
          label="GEN VER"
          value={open === null ? null : formatNumber(open.generator_version, 0)}
        />
        <Reading label="SEED" value={open === null ? null : formatHex64(open.seed)} wide />
        <Reading label="ID" value={open === null ? null : formatHex64(open.id)} wide />
      </dl>
    </>
  );
}

interface UniverseRowProps {
  readonly universe: UniverseInfo;
  readonly isOpen: boolean;
  /** Whether this row's `OPEN` was given and its answer is awaited. */
  readonly isPending: boolean;
  readonly serverGeneratorVersion: number;
  /** The IDs of the elements that say why every command is held back, if anything does. */
  readonly inhibitedBy: ReadonlyArray<string>;
  readonly onOpen: (universe: UniverseIdHex) => void;
  /** Called when the operator points at or focuses this row's `OPEN`, and with `null` after. */
  readonly onPoint: (universe: UniverseIdHex | null) => void;
}

function UniverseRow({
  universe,
  isOpen,
  isPending,
  serverGeneratorVersion,
  inhibitedBy,
  onOpen,
  onPoint,
}: UniverseRowProps) {
  const reasonId = useId();
  const reason = mismatchReason(universe, serverGeneratorVersion);
  const inhibited = reason !== null || inhibitedBy.length > 0;
  return (
    <tr
      className={isOpen ? "universe-table__row universe-table__row--open" : "universe-table__row"}
    >
      <td className="universe-table__name">{universe.name}</td>
      <td className="universe-table__seed">{formatHex64(universe.seed)}</td>
      <td className="universe-table__version">{formatNumber(universe.generator_version, 0)}</td>
      <td className="universe-table__action">
        {isOpen ? (
          <span className="universe-table__current" aria-current="true">
            OPEN
          </span>
        ) : (
          <button
            type="button"
            className="command"
            aria-label={`Open universe ${universe.name}`}
            // Held back rather than disabled, so that it keeps focus and can say why.
            aria-disabled={inhibited ? "true" : undefined}
            aria-describedby={describedBy([reason === null ? null : reasonId, ...inhibitedBy])}
            onClick={() => {
              if (!inhibited) {
                onOpen(universe.id);
              }
            }}
            onFocus={() => {
              onPoint(universe.id);
            }}
            onBlur={() => {
              onPoint(null);
            }}
            // Pointing is a shortcut only: focus, which a tap also gives, shows the same reason.
            onPointerEnter={() => {
              onPoint(universe.id);
            }}
            onPointerLeave={() => {
              onPoint(null);
            }}
          >
            {isPending ? "PENDING" : "OPEN"}
          </button>
        )}
        {reason === null ? null : (
          // The description of the button; the panel shows it below the list when pointed at.
          <span id={reasonId} hidden>
            {reason}
          </span>
        )}
      </td>
    </tr>
  );
}

interface UniverseTableProps {
  readonly universes: ReadonlyArray<UniverseInfo>;
  readonly openId: UniverseIdHex | null;
  /** The universe whose `OPEN` awaits its answer, if any. */
  readonly pendingId: UniverseIdHex | null;
  readonly serverGeneratorVersion: number;
  readonly inhibitedBy: ReadonlyArray<string>;
  readonly onOpen: (universe: UniverseIdHex) => void;
}

/**
 * The server's universes, each with its `OPEN` command, scrolling inside the panel, with the
 * position of the rows in view.
 *
 * @remarks
 * When a universe of another generator version is listed, a line below the list says why the
 * `OPEN` being pointed at or focused is held back, so that the reason is never clipped by the
 * list's edge.
 */
function UniverseTable({
  universes,
  openId,
  pendingId,
  serverGeneratorVersion,
  inhibitedBy,
  onOpen,
}: UniverseTableProps) {
  const { ref, metrics } = useScrollMetrics();
  const [pointed, setPointed] = useState<UniverseIdHex | null>(null);
  const rowPx = ROW_HEIGHT_REM * metrics.remPx;
  // The header stays in view above the rows, so the rows share what is left of the viewport.
  const range = windowRange(
    metrics.scrollTopPx,
    rowPx,
    metrics.viewportPx - HEADER_HEIGHT_REM * metrics.remPx,
    universes.length,
    0,
  );
  const anyMismatch = universes.some((universe) => universe.status === "generator_mismatch");
  const pointedUniverse = universes.find((universe) => universe.id === pointed);
  const pointedReason =
    pointedUniverse === undefined ? null : mismatchReason(pointedUniverse, serverGeneratorVersion);
  return (
    <>
      <div className="universe-panel__list" ref={ref}>
        <table className="universe-table" aria-label="Universes">
          <thead>
            <tr className="universe-table__row">
              <th scope="col" className="universe-table__name">
                NAME
              </th>
              <th scope="col" className="universe-table__seed">
                SEED
              </th>
              <th scope="col" className="universe-table__version">
                GEN VER
              </th>
            </tr>
          </thead>
          <tbody>
            {universes.map((universe) => (
              <UniverseRow
                key={universe.id}
                universe={universe}
                isOpen={universe.id === openId}
                isPending={universe.id === pendingId}
                serverGeneratorVersion={serverGeneratorVersion}
                inhibitedBy={inhibitedBy}
                onOpen={onOpen}
                onPoint={setPointed}
              />
            ))}
          </tbody>
        </table>
      </div>
      {anyMismatch ? (
        // Read by assistive technology as each button's description, so hidden from it here.
        <p className="universe-panel__reason" aria-hidden="true">
          {pointedReason ?? ""}
        </p>
      ) : null}
      {range === null ? null : (
        <p className="list-position">
          {formatListPosition(range.firstVisible, range.lastVisible, universes.length)}
        </p>
      )}
    </>
  );
}

interface UniverseListProps {
  readonly list: RequestState<"list_universes">;
  readonly openId: UniverseIdHex | null;
  readonly pendingId: UniverseIdHex | null;
  readonly inhibitedBy: ReadonlyArray<string>;
  readonly onOpen: (universe: UniverseIdHex) => void;
  readonly onRetry: () => void;
}

/** The universe table, or why there is none: an empty list, or the request's state. */
function UniverseList({
  list,
  openId,
  pendingId,
  inhibitedBy,
  onOpen,
  onRetry,
}: UniverseListProps) {
  if (list.kind === "link_down") {
    // The panel states the link's reason once, above.
    return null;
  }
  if (list.kind !== "ok") {
    return <RequestStatus state={list} onRetry={onRetry} />;
  }
  const { universes, server_generator_version: serverGeneratorVersion } = list.response;
  if (universes.length === 0) {
    return <p className="panel__empty">NO UNIVERSES: create one below</p>;
  }
  return (
    <UniverseTable
      universes={universes}
      openId={openId}
      pendingId={pendingId}
      serverGeneratorVersion={serverGeneratorVersion}
      inhibitedBy={inhibitedBy}
      onOpen={onOpen}
    />
  );
}

/**
 * The `UNIVERSE` panel: the open universe, the server's universes to open, and the form that
 * creates one.
 *
 * @remarks
 * Opening and creating change what the server holds, so both are commands: each shows `PENDING`
 * beside the control that gave it and then the server's answer, never an optimistic change, and
 * no other command can be given until it has answered: the `OPEN` given reads `PENDING`, and
 * `CREATE` has its status beside it. While the link is down every command is held back and states
 * the link's reason, as is the `OPEN` of a universe made by another generator version, which states
 * both versions. Seeds and IDs are shown in upper case. The `NEW UNIVERSE` fields are shown while
 * no universe is open and folded once one is, since universes are created rarely and the
 * parameters below need the room; the operator can show or fold them at any time, and that choice
 * holds until another universe is opened.
 */
export function UniversePanel() {
  const titleId = useId();
  const linkReasonId = useId();
  const commandStatusId = useId();
  const { status } = useServerLink();
  const { open, list, command, create, openUniverse, refresh } = useUniverse();
  const [lastCommand, setLastCommand] = useState<LastCommand>(null);
  const [formChoice, setFormChoice] = useState<FormChoice | null>(null);
  const openId = open?.id ?? null;
  const formExpanded =
    formChoice !== null && formChoice.openId === openId ? formChoice.expanded : openId === null;
  const linkReason = linkDownReason(status);
  const commandPending = command.kind === "pending";
  const inhibitedBy = [
    ...(linkReason === null ? [] : [linkReasonId]),
    ...(commandPending ? [commandStatusId] : []),
  ];

  return (
    <section className="panel galaxy__universe universe-panel" aria-labelledby={titleId}>
      <h2 className="panel__title" id={titleId}>
        Universe
      </h2>
      {linkReason === null ? null : (
        <p className="panel__inhibit" id={linkReasonId}>
          {linkReason}
        </p>
      )}
      <OpenUniverseReadout open={open} />
      {lastCommand?.kind === "open" ? <RequestStatus state={command} id={commandStatusId} /> : null}
      <UniverseList
        list={list}
        openId={openId}
        pendingId={lastCommand?.kind === "open" && commandPending ? lastCommand.universe : null}
        inhibitedBy={inhibitedBy}
        onOpen={(universe) => {
          setLastCommand({ kind: "open", universe });
          openUniverse(universe);
        }}
        onRetry={refresh}
      />
      <NewUniverseForm
        command={lastCommand?.kind === "create" ? command : null}
        commandStatusId={commandStatusId}
        inhibitedBy={inhibitedBy}
        onCreate={(name, seed) => {
          setLastCommand({ kind: "create" });
          create(name, seed);
        }}
        expanded={formExpanded}
        onToggle={() => {
          setFormChoice({ openId, expanded: !formExpanded });
        }}
      />
    </section>
  );
}
