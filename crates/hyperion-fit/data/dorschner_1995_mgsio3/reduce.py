#!/usr/bin/env python3
"""Reduce Dorschner et al.'s (1995) amorphous MgSiO3 to `nk.csv` (plan 14, P14.T49.i).

Reads `pyr-mg100-Dorschner1995.lnk`, optool's copy of the pyroxene glass of Mg/(Mg + Fe) = 1
(`#` comment lines, a line of the row count and the density, then rows of wavelength (um), n and
k), from `crates/hyperion-fit/data/cache/dorschner_1995_mgsio3/` or the path given as the first
argument, and writes beside this file, or to the path given as the second argument, the rows the
task reads: from the last row below 0.78 um to the first row at or above 5 um, each wavelength
(um), n and k as the copy prints them. optool's placeholder k of 10^-6 lies below 0.32 um, outside
these rows.
"""
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "dorschner_1995_mgsio3" / "pyr-mg100-Dorschner1995.lnk"
OUT = HERE / "nk.csv"
FROM_UM, TO_UM = 0.78, 5.0


def main(argv: list[str]) -> None:
    source = pathlib.Path(argv[1]) if len(argv) > 1 else SOURCE
    out = pathlib.Path(argv[2]) if len(argv) > 2 else OUT
    lines = [l for l in source.read_text(encoding="ascii").splitlines() if l.strip() and not l.startswith("#")]
    count = int(lines[0].split()[0])
    rows = [l.split() for l in lines[1:]]
    if len(rows) != count or any(len(r) != 3 for r in rows):
        raise ValueError("the copy's rows are not its stated count of three columns")
    values = [float(r[0]) for r in rows]
    if any(b <= a for a, b in zip(values, values[1:])):
        raise ValueError("the copy's wavelengths do not rise strictly")
    first = max(i for i, v in enumerate(values) if v < FROM_UM)
    last = min(i for i, v in enumerate(values) if v >= TO_UM)
    text = ["wavelength_um,n,k"]
    text += [f"{float(w):.6g},{float(n):.6g},{float(k):.6g}" for w, n, k in rows[first : last + 1]]
    out.write_text("\n".join(text) + "\n", encoding="ascii")


if __name__ == "__main__":
    main(sys.argv)
