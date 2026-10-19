#!/usr/bin/env python3
"""Reduce Grundy and Schmitt's (1998) ice absorption to `alpha_100k.csv` (plan 14, P14.T49.i).

Reads `alpha_H2O.ascii.gz`, the authors' table of hexagonal H2O ice's Lambert absorption
coefficient (cm^-1) at 0.9623-2.742 um every 10 K from 20 to 270 K (columns 2-27, the column
number times 10 being the temperature), from `crates/hyperion-fit/data/cache/grundy_schmitt_1998_h2o/`
or the path given as the first argument, and writes the 100 K column (column 10) beside this file,
or to the path given as the second argument: per row its wavelength (um) and alpha (cm^-1).

The table prints its wavelengths to four significant figures, so above 1 um consecutive rows can
share a printed wavelength (193 do); the rows sharing one are merged into one row whose alpha is
their mean, so that the wavelengths rise strictly. Every other value is the table's own.
"""
import gzip
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "grundy_schmitt_1998_h2o" / "alpha_H2O.ascii.gz"
OUT = HERE / "alpha_100k.csv"
COLUMN = 10  # 1-based: column 10 holds 100 K


def main(argv: list[str]) -> None:
    source = pathlib.Path(argv[1]) if len(argv) > 1 else SOURCE
    out = pathlib.Path(argv[2]) if len(argv) > 2 else OUT
    rows: list[tuple[str, list[float]]] = []
    with gzip.open(source, "rt", encoding="ascii") as f:
        for line in f:
            if line.startswith("#") or not line.strip():
                continue
            cells = line.split()
            if len(cells) != 27:
                raise ValueError(f"a row of {len(cells)} columns, not 27: {line!r}")
            wavelength, alpha = cells[0], float(cells[COLUMN - 1])
            if rows and float(wavelength) == float(rows[-1][0]):
                rows[-1][1].append(alpha)
            else:
                if rows and float(wavelength) < float(rows[-1][0]):
                    raise ValueError(f"the wavelengths fall at {wavelength}")
                rows.append((wavelength, [alpha]))
    lines = ["wavelength_um,alpha_per_cm"]
    for wavelength, alphas in rows:
        lines.append(f"{float(wavelength):.4g},{sum(alphas) / len(alphas):.6g}")
    out.write_text("\n".join(lines) + "\n", encoding="ascii")


if __name__ == "__main__":
    main(sys.argv)
