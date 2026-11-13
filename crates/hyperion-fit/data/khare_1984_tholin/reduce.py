#!/usr/bin/env python3
"""Reduce Khare et al.'s (1984) Titan tholin to `nk.csv` (plan 14, P14.T49.i).

Reads `khare_tholins.dat`, the HITRAN2024 aerosol compilation's ASCII file (`hitran_ri/ascii/exoplanets/khare_tholins.dat`
of `hitran_ri.tar`; a header, then rows of wavenumber (cm^-1), wavelength (um), n and k in falling
wavenumber, so rising wavelength), from `crates/hyperion-fit/data/cache/khare_1984_tholin/` or the path given as the first argument,
and writes beside this file, or to the path given as the second argument, the rows the task reads:
from the last row below 0.78 um to the first row at or above 5 um, each
wavelength (um), n and k as the file prints them.
"""
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "khare_1984_tholin" / "khare_tholins.dat"
OUT = HERE / "nk.csv"
FROM_UM, TO_UM = 0.78, 5.0


def main(argv: list[str]) -> None:
    source = pathlib.Path(argv[1]) if len(argv) > 1 else SOURCE
    out = pathlib.Path(argv[2]) if len(argv) > 2 else OUT
    rows = []
    for line in source.read_text(encoding="ascii").splitlines():
        cells = line.split()
        if len(cells) != 4:
            continue
        try:
            [float(c) for c in cells]
        except ValueError:
            continue
        rows.append(cells[1:])
    values = [float(r[0]) for r in rows]
    if any(b <= a for a, b in zip(values, values[1:])):
        raise ValueError("the file's wavelengths do not rise strictly")
    first = max(i for i, v in enumerate(values) if v < FROM_UM)
    last = min(i for i, v in enumerate(values) if v >= TO_UM)
    text = ["wavelength_um,n,k"]
    text += [f"{float(w):.6g},{float(n):.6g},{float(k):.6g}" for w, n, k in rows[first : last + 1]]
    out.write_text("\n".join(text) + "\n", encoding="ascii")


if __name__ == "__main__":
    main(sys.argv)
