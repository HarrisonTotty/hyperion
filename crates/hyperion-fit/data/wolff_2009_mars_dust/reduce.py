#!/usr/bin/env python3
"""Reduce Wolff et al.'s (2009) Mars dust indices to `nk.csv` (plan 14, P14.T49.i).

Reads `Dust_Refractive_Indicies.txt`, the NASA Ames Mars Climate Modeling Group's table (one header
line, then rows of wavelength (um), n and k, wavelength rising, CRLF line ends), from
`crates/hyperion-fit/data/cache/wolff_2009_mars_dust/` or the path given as the first argument, and
writes beside this file, or to the path given as the second argument, the rows the task reads: from
the last row below 0.78 um to the first row at or above 5 um, each wavelength (um), n and k as the
table prints them. Of the table's 229 rows that is 148, 0.7-5.5 um: its 146 infrared rows over
0.8-4.9946 um and the two that bracket 0.78-5 um. No other row is written.
"""
import hashlib
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "wolff_2009_mars_dust" / "Dust_Refractive_Indicies.txt"
OUT = HERE / "nk.csv"
SHA256 = "bf6069ad22fbfbbef7bf957d386a8177647c87fd98ed6574b5c5f8d6c8016769"
FROM_UM, TO_UM = 0.78, 5.0


def main(argv: list[str]) -> None:
    source = pathlib.Path(argv[1]) if len(argv) > 1 else SOURCE
    out = pathlib.Path(argv[2]) if len(argv) > 2 else OUT
    raw = source.read_bytes()
    if hashlib.sha256(raw).hexdigest() != SHA256:
        raise ValueError("the table is not the one this reduction was written for")
    rows = []
    for line in raw.decode("ascii").splitlines():
        cells = line.split()
        if len(cells) != 3:
            continue
        try:
            [float(c) for c in cells]
        except ValueError:
            continue
        rows.append(cells)
    values = [float(r[0]) for r in rows]
    if any(b <= a for a, b in zip(values, values[1:])):
        raise ValueError("the table's wavelengths do not rise strictly")
    first = max(i for i, v in enumerate(values) if v < FROM_UM)
    last = min(i for i, v in enumerate(values) if v >= TO_UM)
    text = ["wavelength_um,n,k"]
    text += [f"{float(w):.6g},{float(n):.6g},{float(k):.6g}" for w, n, k in rows[first : last + 1]]
    out.write_text("\n".join(text) + "\n", encoding="ascii")


if __name__ == "__main__":
    main(sys.argv)
