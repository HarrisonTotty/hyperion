#!/usr/bin/env python3
"""Reduce Palik and Addamiano's (1985) cubic ZnS, as Kitzmann and Heng (2018) compile it, to
`nk.csv` (plan 14, P14.T49.i).

Reads `ZnS.dat`, LX-MIE's `compilation/ZnS.dat` (`#` header lines, then tab-separated rows of
wavelength (um), n and k), from `crates/hyperion-fit/data/cache/palik_1985_zns/` or the path given
as the first argument, and writes beside this file, or to the path given as the second argument,
the rows the task reads, a reduced near-infrared subset of the one material: from the last row
below 0.78 um to the first row at or above 5 um, each wavelength (um), n and k. No compilation file
is committed whole (decision-composition §5).
"""
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "palik_1985_zns" / "ZnS.dat"
OUT = HERE / "nk.csv"
FROM_UM, TO_UM = 0.78, 5.0


def main(argv: list[str]) -> None:
    source = pathlib.Path(argv[1]) if len(argv) > 1 else SOURCE
    out = pathlib.Path(argv[2]) if len(argv) > 2 else OUT
    rows = []
    for line in source.read_text(encoding="ascii").splitlines():
        if line.startswith("#") or not line.strip():
            continue
        cells = line.split()
        if len(cells) != 3:
            raise ValueError(f"a row of {len(cells)} cells, not 3: {line!r}")
        rows.append([float(c) for c in cells])
    if any(b[0] <= a[0] for a, b in zip(rows, rows[1:])):
        raise ValueError("the file's wavelengths do not rise strictly")
    first = max(i for i, r in enumerate(rows) if r[0] < FROM_UM)
    last = min(i for i, r in enumerate(rows) if r[0] >= TO_UM)
    text = ["wavelength_um,n,k"]
    text += [f"{w:.7g},{n:.7g},{k:.7g}" for w, n, k in rows[first : last + 1]]
    out.write_text("\n".join(text) + "\n", encoding="ascii")


if __name__ == "__main__":
    main(sys.argv)
