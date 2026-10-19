#!/usr/bin/env python3
"""Reduce Martonchik, Orton and Appleby's (1984) ammonia-ice optical constants to `nk.csv` (plan
14, P14.T49.i).

Reads `nh3-m-Martonchik1983.lnk`, optool's copy of their Table I (lines of wavelength (um), n and k
after a first line of the row count and the density), from
`crates/hyperion-fit/data/cache/martonchik_1984_nh3/` or the path given as the first argument, and
writes beside this file, or to the path given as the second argument, the rows the task reads: from
the last row below 0.78 um (0.668 um, whose n alone the task reads) to the first row at or above
5 um, each wavelength (um), n and k. The copy prints its wavelengths to three decimals, so some of
the paper's rows (tabulated in wavenumber) share one printed wavelength inside the 3 um band (at
2.969 and 2.970 um); the rows sharing one are merged into one row whose n and k are their means, so
that the wavelengths rise strictly. Every other value is the copy's own.
"""
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "martonchik_1984_nh3" / "nh3-m-Martonchik1983.lnk"
OUT = HERE / "nk.csv"
FROM_UM, TO_UM = 0.78, 5.0


def main(argv: list[str]) -> None:
    source = pathlib.Path(argv[1]) if len(argv) > 1 else SOURCE
    out = pathlib.Path(argv[2]) if len(argv) > 2 else OUT
    lines = [l for l in source.read_text(encoding="ascii").splitlines() if l.strip() and not l.startswith("#")]
    count = int(lines[0].split()[0])
    rows = [[float(c) for c in l.split()] for l in lines[1:]]
    if len(rows) != count or any(len(r) != 3 for r in rows):
        raise ValueError("the copy's rows are not its stated count of three columns")
    merged: list[list[list[float]]] = []
    for row in rows:
        if merged and row[0] == merged[-1][0][0]:
            merged[-1].append(row)
        else:
            if merged and row[0] < merged[-1][0][0]:
                raise ValueError(f"the wavelengths fall at {row[0]}")
            merged.append([row])
    means = [[sum(r[i] for r in group) / len(group) for i in range(3)] for group in merged]
    first = max(i for i, r in enumerate(means) if r[0] < FROM_UM)
    last = min(i for i, r in enumerate(means) if r[0] >= TO_UM)
    text = ["wavelength_um,n,k"]
    text += [f"{w:.6g},{n:.6g},{k:.6g}" for w, n, k in means[first : last + 1]]
    out.write_text("\n".join(text) + "\n", encoding="ascii")


if __name__ == "__main__":
    main(sys.argv)
