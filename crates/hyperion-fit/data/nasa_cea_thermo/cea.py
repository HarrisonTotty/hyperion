"""An independent reader of NASA CEA's thermo.inp (McBride, Zehe and Gordon 2002,
NASA/TP-2002-211556, the record layout of their Appendix C, Table C1, p. 43), for P14.T49.d's
asserted values.

Imported by kf.py with importlib (the scripts run under `python3 -I`). Reads the file as bytes and
decodes ASCII; it never executes anything from the file. It shares no code with `hyperion-fit`'s
reader, `tasks::thermo_equilibrium::parse`.
"""
import math

LN10 = math.log(10.0)


def num(field):
    field = field.strip().replace("D", "E").replace("d", "e")
    return float(field) if field else 0.0


class Record:
    """One species record: its name line, its formula line, its intervals and its raw bytes."""

    def __init__(self, name, comment, nint, code, formula, phase, mw, hf, intervals, raw, section):
        self.name = name
        self.comment = comment
        self.nint = nint
        self.code = code
        self.formula = formula  # [(element upper-case, count)], electron as "E"
        self.phase = phase  # 0 gas, else condensed
        self.mw = mw
        self.hf = hf
        self.intervals = intervals  # [(tlo, thi, a[7], b1, b2)]
        self.raw = raw  # the record's lines, bytes, with their CRLF
        self.section = section

    def lo(self):
        return self.intervals[0][0]

    def hi(self):
        return self.intervals[-1][1]

    def interval(self, t):
        # The first interval whose closed range holds t: at a shared bound, the lower one.
        for iv in self.intervals:
            if iv[0] <= t <= iv[1]:
                return iv
        return None

    def g_over_rt(self, t):
        iv = self.interval(t)
        if iv is None:
            return None
        _, _, a, b1, b2 = iv
        lt = math.log(t)
        h = (-a[0] / t ** 2 + a[1] * lt / t + a[2] + a[3] * t / 2 + a[4] * t ** 2 / 3
             + a[5] * t ** 3 / 4 + a[6] * t ** 4 / 5 + b1 / t)
        s = (-a[0] / (2 * t ** 2) - a[1] / t + a[2] * lt + a[3] * t + a[4] * t ** 2 / 2
             + a[5] * t ** 3 / 3 + a[6] * t ** 4 / 4 + b2)
        return h - s

    def cp_over_r(self, t):
        iv = self.interval(t)
        if iv is None:
            return None
        a = iv[2]
        return (a[0] / t ** 2 + a[1] / t + a[2] + a[3] * t + a[4] * t ** 2 + a[5] * t ** 3
                + a[6] * t ** 4)


def read(path):
    """Every record of the file, in file order, and the file's header lines (through the
    temperature line after `thermo`)."""
    with open(path, "rb") as f:
        data = f.read()
    lines = data.split(b"\n")
    if lines and lines[-1] == b"":
        lines.pop()
    lines = [l + b"\n" for l in lines]
    text = [l.decode("ascii").rstrip("\r\n") for l in lines]
    i = 0
    while not text[i].startswith("thermo"):
        i += 1
    header_end = i + 2
    i = header_end
    records = []
    section = "products"
    while i < len(text):
        line = text[i]
        if line.startswith("END PRODUCTS"):
            section = "reactants"
            i += 1
            continue
        if line.startswith("END REACTANTS"):
            break
        if line.startswith("!") or not line.strip():
            i += 1
            continue
        parts = line.split(None, 1)
        name = parts[0]
        comment = parts[1].strip() if len(parts) > 1 else ""
        r2 = text[i + 1].ljust(80)
        nint = int(r2[0:2])
        code = r2[3:9]
        formula = []
        for k in range(5):
            el = r2[10 + 8 * k: 12 + 8 * k].strip()
            n = num(r2[12 + 8 * k: 18 + 8 * k])
            if el and n != 0.0:
                formula.append((el.upper(), n))
        phase = int(r2[50:52].strip() or 0)
        mw = num(r2[52:65])
        hf = num(r2[65:80])
        j = i + 2
        intervals = []
        if nint == 0:
            j += 1
        for _ in range(nint):
            r3 = text[j].ljust(80)
            tlo, thi = num(r3[0:11]), num(r3[11:22])
            ncoef = int(r3[22])
            exps = [num(r3[23 + 5 * k: 28 + 5 * k]) for k in range(8)]
            assert ncoef == 7 and exps == [-2, -1, 0, 1, 2, 3, 4, 0], (name, ncoef, exps)
            r4, r5 = text[j + 1].ljust(80), text[j + 2].ljust(80)
            a = [num(r4[16 * k: 16 * k + 16]) for k in range(5)] + [num(r5[0:16]), num(r5[16:32])]
            intervals.append((tlo, thi, a, num(r5[48:64]), num(r5[64:80])))
            j += 3
        records.append(Record(name, comment, nint, code, formula, phase, mw, hf, intervals,
                              b"".join(lines[i:j]), section))
        i = j
    return records, b"".join(lines[:header_end])
