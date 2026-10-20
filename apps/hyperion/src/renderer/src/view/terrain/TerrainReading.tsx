import type { ReactNode } from "react";

import { StaleMark } from "../../components/StaleMark";
import type { ReadoutLine, ReadoutPiece, ReadoutSegment } from "./readouts";

/** The separator of a reading's parts (the guide's "Voice and nomenclature"). */
const PART_SEPARATOR = " · ";

/** A reading line of the terrain model. */
export type TerrainReadingLine = Extract<ReadoutLine, { readonly kind: "reading" }>;

/** Props of {@link TerrainReading}. */
export interface TerrainReadingProps {
  /** The reading: its label and its segments (`readouts.ts`'s reading line). */
  readonly line: TerrainReadingLine;
}

/** A field's tone as its class: stale or Missing, both in `--text-muted` (the guide's "Data states"). */
function toneClass(tone: Extract<ReadoutPiece, { readonly kind: "field" }>["tone"]): string {
  let name: string;
  switch (tone) {
    case "value":
      name = "";
      break;
    case "stale":
      name = " stale";
      break;
    case "missing":
      name = " readout__missing";
      break;
  }
  return name;
}

/** Props of {@link Piece}. */
interface PieceProps {
  readonly piece: ReadoutPiece;
}

/** One piece of a reading's segment, set as `readouts.ts` types it. */
function Piece({ piece }: PieceProps) {
  let node: ReactNode;
  switch (piece.kind) {
    case "field": {
      const className = `terrain-field${piece.align === "start" ? " terrain-field--start" : ""}${toneClass(piece.tone)}`;
      const style = piece.widthCh > 0 ? { minWidth: `${String(piece.widthCh)}ch` } : undefined;
      // A place kept blank, a resolved reading's uncertainty's, holds no value to read.
      node =
        piece.text.length === 0 && !piece.staleMark ? (
          <span aria-hidden="true" className={className} style={style} />
        ) : (
          <output aria-live="off" className={className} style={style}>
            {piece.text}
            {piece.staleMark ? <StaleMark /> : null}
          </output>
        );
      break;
    }
    case "mark":
      node = <span className="terrain-mark">{piece.stale ? <StaleMark /> : null}</span>;
      break;
    case "text":
    case "status":
      node = piece.text;
      break;
  }
  return node;
}

/** Props of {@link Pieces}. */
interface PiecesProps {
  /** A segment's pieces from the first not yet set. */
  readonly pieces: ReadonlyArray<ReadoutPiece>;
}

/**
 * Pieces end to end, set as a chain of the first and the rest rather than as a list, so that no
 * piece needs a key: a piece's place in its segment is all that tells it from another.
 */
function Chain({ pieces }: PiecesProps) {
  const [first, ...rest] = pieces;
  return first === undefined ? null : (
    <>
      <Piece piece={first} />
      <Chain pieces={rest} />
    </>
  );
}

/** Whether a piece is a value or its `S`'s place, which a value's run holds. */
function inRun(piece: ReadoutPiece): boolean {
  return piece.kind === "field" || piece.kind === "mark";
}

/**
 * A segment's pieces end to end ({@link Chain}), each value with its uncertainty and its `S`'s
 * place set as one run that never breaks (`.view-label__run`, as the upper plate's quantities are),
 * so that a `±` reading or its `S` never starts a line alone; the console's words between them stay
 * breakable at their spaces.
 */
function Pieces({ pieces }: PiecesProps) {
  const [first] = pieces;
  if (first === undefined) {
    return null;
  }
  if (!inRun(first)) {
    return (
      <>
        <Piece piece={first} />
        <Pieces pieces={pieces.slice(1)} />
      </>
    );
  }
  const end = pieces.findIndex((piece) => !inRun(piece));
  return (
    <>
      <span className="view-label__run">
        <Chain pieces={end < 0 ? pieces : pieces.slice(0, end)} />
      </span>
      <Pieces pieces={end < 0 ? [] : pieces.slice(end)} />
    </>
  );
}

/** Props of {@link Segments}. */
interface SegmentsProps {
  /** A reading's segments from the first not yet set. */
  readonly segments: ReadonlyArray<ReadoutSegment>;
}

/**
 * A reading's segments, each kept on one line where it fits, joined by the guide's middle dot: set
 * as a chain of the first and the rest, as {@link Pieces} are, so that a segment's place in its
 * reading is its identity.
 */
function Segments({ segments }: SegmentsProps) {
  const [first, ...rest] = segments;
  return first === undefined ? null : (
    <>
      <span className="view-label__part">
        <Pieces pieces={first} />
      </span>
      {rest.length > 0 ? PART_SEPARATOR : null}
      <Segments segments={rest} />
    </>
  );
}

/**
 * One reading of a view's terrain lines (plan R10, R10.T12): its label, then its parts joined by
 * the guide's middle dot, in the label block's line markup, so that its type and breaking match
 * the upper plate's.
 *
 * @remarks
 * Goes inside a `dl.view-label__lines`. Each part stays on one line where it fits, so that a
 * narrow plate breaks the reading at its middle dots first, as the upper plate's readings break.
 * Each value stands in an `output`, an inline block as wide as its fixed field in `ch` of B612
 * Mono, right-aligned, or left-aligned for an uncertainty after its `±`, never broken; each place
 * kept for a value's `S` is 2 ch wide, blank unless the value is stale, so that a unit and the
 * part after it keep their places from Estimated to resolved and stale. A stale value is muted
 * with its `S` (`StaleMark`), the em dash muted as Missing; the console's own text and a status
 * within a reading (`GRAZING`) are neither. No value is announced as it changes (`aria-live`
 * off), since the readout changes at 4 Hz.
 */
export function TerrainReading({ line }: TerrainReadingProps) {
  return (
    <div className="field view-label__line">
      <dt className="field__label">{line.label}</dt>
      <dd>
        <Segments segments={line.segments} />
      </dd>
    </div>
  );
}
