#!/usr/bin/env python3
"""Reduce Grundy, Schmitt and Quirico's (1993) nitrogen-ice absorption to `alpha_41k.csv` (plan 14,
P14.T49.i).

Reads `alpha_N2.ascii`, the authors' table of N2 ice's absorption coefficient (cm^-1) over its 2.15
um band, 2.0-2.3 um, at 20, 34, 37, 38, 41, 48 and 60 K (columns 2-8), from
`crates/hyperion-fit/data/cache/grundy_1993_n2/` or the path given as the first argument, and
writes beside this file, or to the path given as the second argument, the 41 K column (column 6),
beta-N2 above its 35.6 K transition and the column nearest 40 K: per row its wavelength (um) and
alpha (cm^-1), as the table prints them, negative values (noise about zero) included.
"""
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "grundy_1993_n2" / "alpha_N2.ascii"
OUT = HERE / "alpha_41k.csv"
COLUMN = 6  # 1-based: column 6 holds 41 K


def main(argv: list[str]) -> None:
    source = pathlib.Path(argv[1]) if len(argv) > 1 else SOURCE
    out = pathlib.Path(argv[2]) if len(argv) > 2 else OUT
    header = "T=41 K"
    text = source.read_text(encoding="ascii")
    if header not in text:
        raise ValueError(f"the table's header names no `{header}` column")
    rows = []
    for line in text.splitlines():
        cells = line.split()
        if len(cells) != 8:
            continue
        try:
            values = [float(c) for c in cells]
        except ValueError:
            continue
        rows.append((values[0], values[COLUMN - 1]))
    if any(b[0] <= a[0] for a, b in zip(rows, rows[1:])):
        raise ValueError("the table's wavelengths do not rise strictly")
    out_text = ["wavelength_um,alpha_per_cm"]
    out_text += [f"{w:.6g},{a:.6g}" for w, a in rows]
    out.write_text("\n".join(out_text) + "\n", encoding="ascii")


if __name__ == "__main__":
    main(sys.argv)
