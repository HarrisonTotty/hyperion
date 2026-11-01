/**
 * The galaxy's accreted progenitors as the `PARAMETERS` page lists them (plan 10, P10.T11): one
 * group a progenitor, after the parameters' groups, each reading as a parameter reads.
 */
import type { AccretedProgenitor } from "../../lib/galaxy/model";
import { DWARF_CORE_LABEL, STREAM_LABEL } from "../../lib/galaxy/streamTracks";
import { numberText, type ParameterReading } from "./parameterReadings";

/** One progenitor's group: its heading and its readings. */
export interface ProgenitorGroup {
  readonly key: string;
  readonly label: string;
  readonly readings: ReadonlyArray<ParameterReading>;
}

/** What a progenitor left on the global list, in words: `STREAM 7 · DWARF CORE 0`, or `NONE`. */
function leftText({ stream, core }: AccretedProgenitor): string {
  const left = [
    stream === null ? null : `${STREAM_LABEL} ${String(stream)}`,
    core === null ? null : `${DWARF_CORE_LABEL} ${String(core)}`,
  ].filter((part) => part !== null);
  return left.length === 0 ? "NONE" : left.join(" · ");
}

/**
 * The progenitors' groups, in the list's order: the last major merger, `DOMINANT MERGER`, then the
 * recent dwarfs in their history's order, `RECENT DWARF 0` onwards, numbered from 0 as the list's
 * streams and cores are; each with its `STELLAR MASS`,
 * how long before the epoch it was accreted (`ACCRETED AGO`, in `Gyr`), its orbit's `PERICENTRE`
 * and `APOCENTRE` about the galactic centre, drawn as plan 02 draws the accretion history, and what
 * it `LEFT` on the global list, derived.
 */
export function progenitorGroups(
  progenitors: ReadonlyArray<AccretedProgenitor>,
): ReadonlyArray<ProgenitorGroup> {
  let recent = 0;
  return progenitors.map((progenitor) => {
    let label: string;
    let key: string;
    switch (progenitor.kind) {
      case "dominant_merger":
        label = "DOMINANT MERGER";
        key = "progenitor.dominant_merger";
        break;
      case "recent":
        // Its place in the accretion history, which the list keeps in order: `ProgenitorKind`'s j.
        label = `RECENT DWARF ${String(recent)}`;
        key = `progenitor.recent.${String(recent)}`;
        recent += 1;
        break;
    }
    const drawn = (
      name: string,
      readingLabel: string,
      value: number,
      unit: "msun" | "gyr" | "ly",
    ): ParameterReading => ({
      key: `${key}.${name}`,
      label: readingLabel,
      text: numberText(value, unit),
      unit,
      origin: "DRAWN",
    });
    return {
      key,
      label,
      readings: [
        drawn("mass", "STELLAR MASS", progenitor.massMsun, "msun"),
        drawn("accreted", "ACCRETED AGO", progenitor.accretedGyr, "gyr"),
        drawn("pericentre", "PERICENTRE", progenitor.pericentreLy, "ly"),
        drawn("apocentre", "APOCENTRE", progenitor.apocentreLy, "ly"),
        {
          key: `${key}.left`,
          label: "LEFT",
          text: leftText(progenitor),
          unit: null,
          origin: "DERIVED",
        },
      ],
    };
  });
}
