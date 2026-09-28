import { binaryClassLabel } from "../lib/galaxy/binaryClass";
import { formatMassMsun, formatOrbit } from "../lib/format";
import type { SystemModel } from "../lib/system/model";
import { type StarListStar, starListRows } from "../lib/system/starList";
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
 * one row and no orbit table. A third table names the class of the interacting binary each star
 * belongs to in words (`DWARF NOVA`), under the star's letter, and reads `NONE` when no star is in
 * one; while the server computes no class (until plan 11's P11.T11) the list says so once,
 * `BINARY CLASSES: NOT YET MODELLED`, rather than a column of em dashes, so that the tables keep
 * the readout's width. Numbers are right-aligned in their columns. A stale answer is muted, and each table's
 * title trails the guide's `S`.
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
      <BinaryClasses stars={stars} stale={stale} />
    </div>
  );
}

/**
 * The binary classes of a system's stars: each star in a class under its letter, `NONE` when no
 * star is in one, and the section's `NOT YET MODELLED` while the server computes none.
 */
interface BinaryClassesProps {
  readonly stars: ReadonlyArray<StarListStar>;
  /** Whether the answer is a stale snapshot. */
  readonly stale: boolean;
}

function BinaryClasses({ stars, stale }: BinaryClassesProps) {
  if (stars.every(({ host }) => host.binaryClass.kind === "not_modelled")) {
    return (
      <p className="star-list__note">
        BINARY CLASSES: NOT YET MODELLED{stale ? <StaleMark /> : null}
      </p>
    );
  }
  const classed = stars.flatMap(({ letter, host }) =>
    host.binaryClass.kind === "none"
      ? []
      : [{ letter, id: host.id, binaryClass: host.binaryClass }],
  );
  return (
    <table className="star-list__table">
      <caption className="star-list__caption">BINARY CLASSES{stale ? <StaleMark /> : null}</caption>
      <thead>
        <tr>
          <th scope="col">STAR</th>
          <th scope="col">BINARY CLASS</th>
        </tr>
      </thead>
      <tbody>
        {classed.length === 0 ? (
          <tr>
            <td colSpan={2}>NONE</td>
          </tr>
        ) : (
          classed.map(({ letter, id, binaryClass }) => (
            <tr key={id}>
              <th scope="row">{letter}</th>
              <td>
                {binaryClass.kind === "value" ? (
                  binaryClassLabel(binaryClass.value)
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
