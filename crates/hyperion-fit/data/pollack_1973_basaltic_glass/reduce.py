#!/usr/bin/env python3
"""Reduce Pollack, Toon and Khare's (1973) basaltic glass to `nk.csv` (plan 14, P14.T49.i).

Reads `basaltic_glass_R_Pollack_1973.ri`, the Aerosol Refractive Index Archive's file (`#` header lines, then tab-separated
rows of wavelength (um), n and k, `NaN` where the paper gives no value), from
`crates/hyperion-fit/data/cache/pollack_1973_basaltic_glass/` or the path given as the first argument, and writes beside
this file, or to the path given as the second argument, the rows the task reads: from the last row
below 0.78 um to the first row at or above 5 um that give each of n and k, each wavelength (um), n
and k as the file prints them, `NaN` kept, so that n is read from the rows that give n and k from the rows that give k.
"""
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "pollack_1973_basaltic_glass" / "basaltic_glass_R_Pollack_1973.ri"
OUT = HERE / "nk.csv"
FROM_UM, TO_UM = 0.78, 5.0


def main(argv: list[str]) -> None:
    source = pathlib.Path(argv[1]) if len(argv) > 1 else SOURCE
    out = pathlib.Path(argv[2]) if len(argv) > 2 else OUT
    rows = []
    for line in source.read_text(encoding="ascii").splitlines():
        if line.startswith("#") or not line.strip():
            continue
        cells = line.split("\t")
        if len(cells) != 3:
            raise ValueError(f"a row of {len(cells)} cells, not 3: {line!r}")
        rows.append([c.strip() for c in cells])
    values = [float(r[0]) for r in rows]
    if any(b <= a for a, b in zip(values, values[1:])):
        raise ValueError("the file's wavelengths do not rise strictly")
    # Each of n and k is bracketed by its own rows: the last below 0.78 um and the first at or above
    # 5 um that give it.
    given = lambda r, col: r[col] != "NaN"
    first = min(max(i for i, r in enumerate(rows) if values[i] < FROM_UM and given(r, col)) for col in (1, 2))
    last = max(min(i for i, r in enumerate(rows) if values[i] >= TO_UM and given(r, col)) for col in (1, 2))
    text = ["wavelength_um,n,k"]
    text += [",".join(f"{float(c):.6g}" if c != "NaN" else "NaN" for c in r) for r in rows[first : last + 1]]
    out.write_text("\n".join(text) + "\n", encoding="ascii")


if __name__ == "__main__":
    main(sys.argv)
