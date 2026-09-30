import { Fragment } from "react";

import { binaryClassLabel, binaryClassRows } from "../lib/galaxy/binaryClass";
import { formatMassMsun, formatOrbit } from "../lib/format";
import type { SystemModel } from "../lib/system/model";
import { starListRows } from "../lib/system/starList";
import { objectKindLabel } from "../lib/system/words";
import { SolarMassUnit } from "./SolarMassUnit";
import { StaleMark } from "./StaleMark";

/** Props of {@link StarList}. */
export interface StarListProps {
  /** The system, as `system_summary` answered for it. */
  readonly model: SystemModel;
  /** Whether the answer is a stale snapshot: every value muted, each title with the guide's `S`. */
  readonly stale: boolean;
}

/**
 * Every star of a system and every orbit that holds them, as two small tables (plan 11, P11.T14).
 *
 * @remarks
 * Each star is read under its component letter, `A` for the primary and then `B`, `C` in hierarchy
 * order: its class as an astronomer writes it, its mass now in `M☉` with the drawn `☉`, and what it
 * is in words (`DWARF`, `WHITE DWARF`), so that its state never rests on a symbol. A star that left
 * no remnant has no mass to read, and its mass is the em dash. Each orbit is named by the letters it
 * joins, `A–B` or `AB–C`, the outermost first, and reads its period, its semi-major axis and its
 * eccentricity, each as every orbit on the ship is read (`formatOrbit`). A single star's list has
 * one row and no orbit table. A third table names the class of each interacting binary in words
 * (`DWARF NOVA`), under the stars that carry it (`A–B` for an innermost pair, `A` for a star that
 * carries a class alone, ruling 130.1), and reads `NONE` when no star is in one; while the
 * server computes no class (until plan 11's P11.T11) the list says so once,
 * `BINARY CLASSES: NOT YET MODELLED`, rather than a column of em dashes, so that the tables keep
 * the readout's width. Numbers are right-aligned in their columns. A stale answer is muted, and
 * each table's title trails the guide's `S`.
 */
export function StarList({ model, stale }: StarListProps) {
  const { stars, orbits } = starListRows(model);
  return (
    <div className={stale ? "star-list stale" : "star-list"}>
      <table className="star-list__table">
        <caption className="star-list__caption">STARS{stale ? <StaleMark /> : null}</caption>
        <thead>
          <tr>
            <th scope="col">STAR</th>
            <th scope="col">CLASS</th>
            <th scope="col" className="star-list__number">
              MASS <SolarMassUnit />
            </th>
            <th scope="col">STATE</th>
          </tr>
        </thead>
        <tbody>
          {stars.map(({ letter, host }) => (
            <tr key={host.id}>
              <th scope="row">{letter}</th>
              <td className="star-list__value">{host.spectralClass}</td>
              <td className="star-list__number">
                {host.kind === "no_remnant" ? (
                  <span className="readout__missing">—</span>
                ) : (
                  formatMassMsun(host.massMsun)
                )}
              </td>
              <td>{objectKindLabel(host.kind)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {orbits.length === 0 ? null : (
        <table className="star-list__table">
          <caption className="star-list__caption">ORBITS{stale ? <StaleMark /> : null}</caption>
          <thead>
            <tr>
              <th scope="col">ORBIT</th>
              <th scope="col" className="star-list__number">
                PERIOD
              </th>
              <th scope="col" className="star-list__number">
                SMA
              </th>
              <th scope="col" className="star-list__number">
                ECC
              </th>
            </tr>
          </thead>
          <tbody>
            {orbits.map(({ id, label, orbit }) => {
              const read = formatOrbit(orbit.periodS, orbit.semiMajorAxisM, orbit.eccentricity);
              return (
                <tr key={id}>
                  <th scope="row">{label}</th>
                  <td className="star-list__number">
                    {read.period.value} <span className="star-list__unit">{read.period.unit}</span>
                  </td>
                  <td className="star-list__number">
                    {read.semiMajorAxis.value}{" "}
                    <span className="star-list__unit">{read.semiMajorAxis.unit}</span>
                  </td>
                  <td className="star-list__number">{read.eccentricity}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
      <BinaryClasses model={model} stale={stale} />
    </div>
  );
}

/** Props of {@link Words}. */
interface WordsProps {
  /** The label, in words separated by single spaces. */
  readonly text: string;
}

/**
 * A label that may wrap at its spaces but never after a hyphen: each hyphenated word is kept whole
 * (the orchestrator's ruling 130.5; B612 has no non-breaking hyphen).
 */
function Words({ text }: WordsProps) {
  // No class's label repeats a word (binaryClass.test.ts holds them to it), so each word keys itself.
  return text.split(" ").map((word, index) => (
    <Fragment key={word}>
      {index > 0 ? " " : null}
      {word.includes("-") ? <span className="star-list__whole">{word}</span> : word}
    </Fragment>
  ));
}

/** Props of {@link BinaryClasses}. */
interface BinaryClassesProps {
  /** The system, as `system_summary` answered for it. */
  readonly model: SystemModel;
  /** Whether the answer is a stale snapshot. */
  readonly stale: boolean;
}

/**
 * The binary classes of a system: one row to an innermost pair that shares its class (`A–B`) or to
 * a star that carries one alone (`A`), `NONE` when no star is in one, and the section's
 * `NOT YET MODELLED` while the server computes none (the orchestrator's rulings 130.1 and 130.6).
 */
function BinaryClasses({ model, stale }: BinaryClassesProps) {
  const table = binaryClassRows(model);
  if (table.kind === "not_modelled") {
    return (
      <p className="star-list__note">
        BINARY CLASSES: NOT YET MODELLED{stale ? <StaleMark /> : null}
      </p>
    );
  }
  return (
    <table className="star-list__table">
      <caption className="star-list__caption">BINARY CLASSES{stale ? <StaleMark /> : null}</caption>
      <thead>
        <tr>
          <th scope="col">STARS</th>
          <th scope="col">BINARY CLASS</th>
        </tr>
      </thead>
      <tbody>
        {table.rows.length === 0 ? (
          <tr>
            <td colSpan={2}>NONE</td>
          </tr>
        ) : (
          table.rows.map(({ id, stars, binaryClass }) => (
            <tr key={id}>
              <th scope="row">{stars}</th>
              <td>
                {binaryClass.kind === "value" ? (
                  <Words text={binaryClassLabel(binaryClass.value)} />
                ) : (
                  <span className="readout__missing">—</span>
                )}
              </td>
            </tr>
          ))
        )}
      </tbody>
    </table>
  );
}
