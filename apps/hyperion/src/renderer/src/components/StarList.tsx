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
 * one row and no orbit table. Binary classes are not on the wire yet (P11.T5 and T13), so the list
 * names none. Numbers are right-aligned in their columns. A stale answer is muted, and each table's
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
    </div>
  );
}
