#!/usr/bin/env python3
"""Reduce NASA CEA's `thermo.inp` to the records P14.T49.d reads, kept verbatim (plan 14,
P14.T49.d; `thermo_equilibrium`).

Reads `thermo.inp` (McBride, Zehe and Gordon 2002, NASA/TP-2002-211556, Appendix A's layout) of
NASA's `nasa/cea` repository at commit 2f79a647e85737742fb08a127da00d1ea1d069da, from
`crates/hyperion-fit/data/cache/nasa_cea_thermo/` or the path given as the first argument, checks
its SHA-256, and writes beside this file, or to the path given as the second argument, a library
in the same layout holding only:

- every substance-registry gas row's record (FeH has none);
- the reference elements' records: the gases H2, N2, O2, CL2 and the noble gases, the electron
  gas e-, graphite, every other element's condensed Ref-Elm records and its gas form (the monatomic
  gas for the metals, S2 for sulphur, P2 for phosphorus);
- the records P14.T49.d's cross-checks read: MgSO4(II), MgSO4(I), MgO(cr) and SO3 for MgSO4's
  decomposition, and C, C2, C3, C4 and C5 over graphite for its sublimation.

The file's header (its comment block, the `thermo` line and the temperature line after it) and
every selected record are copied byte for byte, with their CRLF line ends, in the file's order,
followed by the file's own `END PRODUCTS` and `END REACTANTS` lines. Nothing is edited, rounded or
re-encoded, so the output is a CEA library in its own right. A name the file holds twice
(`Cr(cr)`, `Fe(a)` and `Ni(cr)`, each a condensed phase below and above its lambda transition) is
copied with both its records.
"""
import hashlib
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "nasa_cea_thermo" / "thermo.inp"
OUT = HERE / "thermo.inp"
SOURCE_SHA256 = "fa7746572952d74e249e818a82a35c113829742fb421a308e167185528884363"

# The registry's gas rows, by CEA's names (`AL`, `AL+` and `HCL` in its capitals; HCN, not HNC;
# acetylene, not vinylidene).
ROWS = [
    "H2", "He", "H2O", "CH4", "NH3", "N2", "O2", "CO2", "Ar", "Ne", "Kr", "Xe", "N2O", "CO", "SO2",
    "H2S", "O3", "HCN", "C2H2,acetylene", "C2H4", "C2H6", "PH3", "CH3OH", "H", "O", "e-", "H-",
    "Na", "K", "Fe", "Mg", "Si", "Ca", "Ti", "SiO", "TiO", "VO", "AL", "V", "Mn", "Ni", "Zn",
    "Na+", "Mg+", "AL+", "Si+", "K+", "Ca+", "Ti+", "V+", "Cr+", "Mn+", "Fe+", "Ni+", "Zn+",
    "HCL", "Cr",
]
# The reference elements' records that are not rows.
REFERENCES = [
    "CL2", "S2", "P2", "C(gr)", "Na(cr)", "Na(L)", "K(cr)", "K(L)", "Mg(cr)", "Mg(L)", "AL(cr)",
    "AL(L)", "Si(cr)", "Si(L)", "P(cr)", "P(L)", "S(a)", "S(b)", "S(L)", "Ca(a)", "Ca(b)", "Ca(L)",
    "Ti(a)", "Ti(b)", "Ti(L)", "V(cr)", "V(L)", "Cr(cr)", "Cr(L)", "Mn(a)", "Mn(b)", "Mn(c)",
    "Mn(d)", "Mn(L)", "Fe(a)", "Fe(c)", "Fe(d)", "Fe(L)", "Ni(cr)", "Ni(L)", "Zn(cr)", "Zn(L)",
]
# The cross-checks' records.
CROSS_CHECKS = ["MgSO4(II)", "MgSO4(I)", "MgO(cr)", "SO3", "C", "C2", "C3", "C4", "C5"]


def records(lines: list[bytes]) -> tuple[int, list[tuple[str, int, int]], int, int]:
    """The header's end, each product record's name and line span, and the `END PRODUCTS` and
    `END REACTANTS` lines' indices."""
    text = [line.decode("ascii").rstrip("\r\n") for line in lines]
    i = next(k for k, line in enumerate(text) if line.startswith("thermo")) + 2
    header_end = i
    spans = []
    while not text[i].startswith("END PRODUCTS"):
        if text[i].startswith("!") or not text[i].strip():
            i += 1
            continue
        name = text[i].split()[0]
        intervals = int(text[i + 1][0:2])
        end = i + 2 + (3 * intervals if intervals else 1)
        spans.append((name, i, end))
        i = end
    end_products = i
    end_reactants = next(
        k for k in range(i, len(text)) if text[k].startswith("END REACTANTS")
    )
    return header_end, spans, end_products, end_reactants


def main(argv: list[str]) -> None:
    source = pathlib.Path(argv[1]) if len(argv) > 1 else SOURCE
    out = pathlib.Path(argv[2]) if len(argv) > 2 else OUT
    data = source.read_bytes()
    found = hashlib.sha256(data).hexdigest()
    if found != SOURCE_SHA256:
        raise ValueError(f"{source} has SHA-256 {found}, not {SOURCE_SHA256}")
    lines = data.splitlines(keepends=True)
    header_end, spans, end_products, end_reactants = records(lines)
    wanted = ROWS + REFERENCES + CROSS_CHECKS
    if len(set(wanted)) != len(wanted):
        raise ValueError("a name is selected twice")
    names = {name for name, _, _ in spans}
    missing = [name for name in wanted if name not in names]
    if missing:
        raise ValueError(f"the file holds no record named {missing}")
    chunks = lines[:header_end]
    for name, start, end in spans:
        if name in wanted:
            chunks += lines[start:end]
    chunks += [lines[end_products], lines[end_reactants]]
    out.write_bytes(b"".join(chunks))


if __name__ == "__main__":
    main(sys.argv)
