import { LegendSymbol } from "../../spatial/LegendSymbol";
import type { SymbolShape } from "../../spatial/marks";
import { SIZE_CLASS_REM } from "../../spatial/symbols";

/** One shape of the star symbol set, with its accessible name and what it stands for. */
interface StarShapeEntry {
  readonly shape: SymbolShape;
  /** The shape itself, as a screen reader names the drawn symbol. */
  readonly name: string;
  /** The kinds of primary it is drawn for, in words. */
  readonly meaning: string;
}

/**
 * The five shapes of the star symbol set, in the order of the owner's draft of the guide's table
 * (plan 06, design note 17), each with the kinds `starSymbol` draws it for.
 *
 * @remarks
 * Each names every kind the shape is drawn for, in the words the list and readout use
 * (`objectKindLabel`), as the draft's table does (r9 D4.4).
 */
const STAR_SHAPES: ReadonlyArray<StarShapeEntry> = [
  {
    shape: "circle",
    name: "Circle",
    meaning: "PROTOSTAR, PRE-MAIN-SEQUENCE STAR, DWARF, SUBGIANT, HOT SUBDWARF, BROWN DWARF",
  },
  { shape: "ringed-circle", name: "Ringed circle", meaning: "GIANT, SUPERGIANT, WOLF-RAYET STAR" },
  { shape: "diamond", name: "Diamond", meaning: "WHITE DWARF" },
  { shape: "triangle", name: "Triangle", meaning: "NEUTRON STAR" },
  { shape: "square", name: "Square", meaning: "BLACK HOLE" },
];

/** The shapes of the compact remnants, which a picture that does not draw them leaves out. */
const COMPACT_REMNANT_SHAPES: ReadonlySet<SymbolShape> = new Set(["triangle", "square"]);

/** Props of {@link StarShapeLegend}. */
export interface StarShapeLegendProps {
  /**
   * Whether the picture draws neutron stars and black holes, as the chart does; the HR diagram
   * counts them instead, and its legend names only the shapes it draws.
   */
  readonly compactRemnants: boolean;
}

/**
 * The legend entries of the star symbol set: each shape, drawn from the outline the views paint it
 * with and named for assistive technology, beside the kinds it stands for.
 *
 * @remarks
 * Items of a legend group, not a group of their own, so that the local chart's legend and the HR
 * diagram's each hold them among their other entries. A system whose star left no remnant, or that
 * is not yet formed, has no symbol, which the last entry says, so that its absence from a picture
 * is not read as an empty sky.
 */
export function StarShapeLegend({ compactRemnants }: StarShapeLegendProps) {
  const shown = compactRemnants
    ? STAR_SHAPES
    : STAR_SHAPES.filter((entry) => !COMPACT_REMNANT_SHAPES.has(entry.shape));
  return (
    <>
      {shown.map(({ shape, name, meaning }) => (
        // The circle's six kinds are longer than a narrow column, so its words may wrap.
        <p className="symbol-legend__item symbol-legend__item--wraps" key={shape}>
          <LegendSymbol shape={shape} diameterRem={SIZE_CLASS_REM[4]} filled name={name} />
          {meaning}
        </p>
      ))}
      <p className="symbol-legend__item">NO REMNANT, NOT YET FORMED: LIST ONLY</p>
    </>
  );
}
