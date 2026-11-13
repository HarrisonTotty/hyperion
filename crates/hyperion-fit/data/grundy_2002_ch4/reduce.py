#!/usr/bin/env python3
"""Reduce Grundy, Schmitt and Quirico's (2002) methane-ice absorption to `alpha_40k.csv` (plan 14,
P14.T49.i).

Reads `alpha_CH4.ascii`, the authors' reduced-resolution table of CH4 ice's Lambert absorption
coefficient (cm^-1) at 0.648-4.913 um and at 20, 30, 40, 50, 60, 70, 80, 90 and 93 K (columns 2-10),
from `crates/hyperion-fit/data/cache/grundy_2002_ch4/` or the path given as the first argument,
and writes beside this file, or to the path given as the second argument, the 40 K column
(column 4), phase I, from the last row below 0.78 um to the table's end: per row its wavelength (um)
and alpha (cm^-1), as the table prints them, negative values (noise about zero) included.
"""
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "grundy_2002_ch4" / "alpha_CH4.ascii"
OUT = HERE / "alpha_40k.csv"
COLUMN = 4  # 1-based: column 4 holds 40 K
FROM_UM = 0.78


def main(argv: list[str]) -> None:
    source = pathlib.Path(argv[1]) if len(argv) > 1 else SOURCE
    out = pathlib.Path(argv[2]) if len(argv) > 2 else OUT
    rows = []
    for line in source.read_text(encoding="ascii").splitlines():
        cells = line.split()
        if len(cells) != 10:
            continue
        try:
            values = [float(c) for c in cells]
        except ValueError:
            continue
        rows.append((cells[0], cells[COLUMN - 1], values[0]))
    if any(b[2] <= a[2] for a, b in zip(rows, rows[1:])):
        raise ValueError("the table's wavelengths do not rise strictly")
    first = max(i for i, r in enumerate(rows) if r[2] < FROM_UM)
    text = ["wavelength_um,alpha_per_cm"]
    text += [f"{float(w):.6g},{float(a):.6g}" for w, a, _ in rows[first:]]
    out.write_text("\n".join(text) + "\n", encoding="ascii")


if __name__ == "__main__":
    main(sys.argv)
